/// NOTE: Yes this is AI generated. No I don't care this silly example is
/// AI generated. It serves only one purpose and that's to stress and
/// abuse my API I've created, which this test does very well.
/// # QUIC Concurrency Stress Test
///
/// Benchmarks how many concurrent clients can continuously send data to a
/// single server without dropping below 60 FPS.
///
/// Unlike `quic_benchmark.rs`, this example measures real frame time in
/// `Update` (where `Time::delta()` reflects wall-clock time, not the fixed
/// timestep).  All management systems are chained in `Update` to avoid the
/// timing desync between fixed-step and variable-step schedules that can
/// cause missed wave reports at high client counts.
///
/// ## How it works
///
/// - A server is bound to a loopback address.
/// - Every `RAMP_INTERVAL_SECS` seconds a wave of `CLIENTS_PER_WAVE`
///   clients is spawned.  Each client opens a bidirectional stream and
///   sends a fixed payload every fixed-update tick.
/// - The server counts received bytes.
/// - Real frame time is checked every `Update`.  If any frame exceeds
///   the 60 FPS budget (~16.67 ms) the benchmark marks the threshold as
///   breached and exits after reporting.
/// - Results are printed after every wave and in a final summary table.
///
/// ## Usage
///
/// ```ignore
/// cargo run --example quic_concurrency_stress --release
/// ```
use bevy::{
    DefaultPlugins,
    app::{
        App, AppExit, FixedPostUpdate, Startup, TaskPoolOptions, TaskPoolPlugin,
        TaskPoolThreadAssignmentPolicy, Update,
    },
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::MessageWriter,
        query::{With, Without},
        resource::Resource,
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res, ResMut},
    },
    input::{ButtonInput, keyboard::KeyCode},
    log::{LogPlugin, error, info, warn},
    prelude::PluginGroup,
    render::{
        RenderPlugin,
        settings::{PowerPreference, WgpuSettings},
    },
    time::Time,
    utils::default,
};
use bevy_s2n_quic::{
    QuicDefaultPlugins,
    client::{QuicClient, marker::QuicClientMarker},
    common::{
        connection::QuicConnection,
        runtime::TokioRuntime,
        stream::{receive::QuicReceiveStream, send::QuicSendStream},
    },
    server::{QuicServer, marker::QuicServerMarker},
};
use s2n_quic::client::Connect;
use std::{
    io::Write,
    net::SocketAddr,
    path::Path,
    time::{Duration, Instant},
};

// ─── Tuning knobs ────────────────────────────────────────────────────────────

/// Server address (loopback only).
const BIND_ADDR: &str = "127.0.0.1:17779";

/// How many new clients per wave.
const CLIENTS_PER_WAVE: usize = 10;

/// Maximum number of waves before the benchmark exits voluntarily.
const MAX_WAVES: usize = 60;

/// Seconds between wave spawns.
const RAMP_INTERVAL_SECS: f64 = 4.0;

/// Fixed payload sent by every client every fixed-update tick.
const PAYLOAD: &[u8] = b"STRESS_PAYLOAD_ABCDEFGHIJ_1234567890";

/// 60 FPS threshold in milliseconds.
const FPS_THRESHOLD_MS: f64 = 1000.0 / FPS_THRESHOLD as f64;

const FPS_THRESHOLD: usize = 10;

/// Set to `true` for manual keyboard-driven mode:
///   - Press  SPACE  to spawn `CLIENTS_PER_WAVE` clients (and measure the previous batch)
///   - Press  ESC    to print the report and exit
///
/// Set to `false` for automatic wave ramp as described above.
const MANUAL_MODE: bool = true;

// ─── Marker components ───────────────────────────────────────────────────────

#[derive(Component)]
struct StressClientStream;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
struct StressBench {
    next_wave_at: Instant,
    current_wave: usize,
    total_clients: usize,
    /// Snapshot of `total_clients` taken *before* spawning the next wave,
    /// so it reflects the client count that existed while the current
    /// batch of frame-time samples was being accumulated.
    prev_wave_client_count: usize,
    frame_times_ms: Vec<f64>,
    last_recorded_wave: usize,
    results: Vec<WaveResult>,
    /// Index (into `results`) of the first failing wave, if any.
    first_fail_index: Option<usize>,
    report_printed: bool,
    start: Instant,
    bytes_this_frame: u64,
    bytes_total: u64,
    /// Set after the last wave has been spawned.  The benchmark keeps
    /// running for one more wave-interval to collect a clean final
    /// measurement, then exits.
    exit_deadline: Option<Instant>,
}

#[derive(Debug, Clone)]
struct WaveResult {
    client_count: usize,
    max_ms: f64,
    min_fps: f64,
    avg_fps: f64,
    rx_avg: u64,
    ok: bool,
}

impl Default for StressBench {
    fn default() -> Self {
        Self {
            next_wave_at: Instant::now(),
            current_wave: 0,
            total_clients: 0,
            prev_wave_client_count: 0,
            frame_times_ms: Vec::new(),
            last_recorded_wave: 0,
            results: Vec::new(),
            first_fail_index: None,
            report_printed: false,
            start: Instant::now(),
            bytes_this_frame: 0,
            bytes_total: 0,
            exit_deadline: None,
        }
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        power_preference: PowerPreference::LowPower,
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                })
                .set(LogPlugin {
                    filter: "info,bevy_quic_networking::common=error".into(),
                    level: bevy::log::Level::INFO,
                    ..Default::default()
                })
                .set(TaskPoolPlugin {
                    task_pool_options: TaskPoolOptions {
                        compute: TaskPoolThreadAssignmentPolicy {
                            min_threads: 1,
                            max_threads: 4, // experiment with this
                            percent: 0.5,
                            on_thread_spawn: None,
                            on_thread_destroy: None,
                        },
                        ..default()
                    },
                }),
        )
        .add_plugins(QuicDefaultPlugins)
        .init_resource::<StressBench>()
        .add_systems(Startup, setup)
        // All management systems run in `Update` and are chained so that
        // `spawn_wave` increments `current_wave` *before* `record_wave`
        // checks whether a wave boundary has just been crossed.
        .add_systems(Update, (spawn_wave, open_streams, record_wave).chain())
        .add_systems(FixedPostUpdate, (client_send, server_recv))
        .run();
}

// ─── Setup ───────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands, runtime: Res<TokioRuntime>) {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let cert_str = format!("{manifest}/examples/certs/cert.pem");
    let key_str = format!("{manifest}/examples/certs/key.pem");
    let cert_path = Path::new(&cert_str);
    let key_path = Path::new(&key_str);
    let ip: SocketAddr = BIND_ADDR.parse().unwrap();

    commands.spawn(
        QuicServer::bind(&runtime, ip, cert_path, key_path)
            .expect("Failed to bind server"),
    );

    if MANUAL_MODE {
        info!(
            "=== QUIC Concurrency Stress Test [MANUAL] ===\n\
             Bind:     {BIND_ADDR}\n\
             Payload:  {} bytes  |  Batch: {CLIENTS_PER_WAVE} clients\n\
             Threshold: {} FPS ({FPS_THRESHOLD_MS:.2}ms)\n\
             \n\
             Controls:\n\
               SPACE  – spawn a batch of {CLIENTS_PER_WAVE} clients\n\
               ESC    – print report and exit\n\
             =============================================",
            PAYLOAD.len(),
            FPS_THRESHOLD
        );
    } else {
        info!(
            "=== QUIC Concurrency Stress Test [AUTO] ===\n\
             Bind:     {BIND_ADDR}\n\
             Payload:  {} bytes\n\
             Wave:     {CLIENTS_PER_WAVE} clients every {RAMP_INTERVAL_SECS}s\n\
             Max:      {MAX_WAVES} waves  |  Threshold: {} FPS ({FPS_THRESHOLD_MS:.2}ms)\n\
             ==========================================",
            PAYLOAD.len(),
            FPS_THRESHOLD
        );
    }
}

// ─── Wave spawning ───────────────────────────────────────────────────────────
//
// In auto mode the function is a no-op once
// `current_wave >= MAX_WAVES` or the first failure is recorded.

fn spawn_wave(
    mut commands: Commands,
    runtime: Res<TokioRuntime>,
    mut state: ResMut<StressBench>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if state.first_fail_index.is_some() {
        return;
    }

    if MANUAL_MODE {
        // ── Manual mode ────────────────────────────────────────────────────
        //   SPACE  – spawn a batch and record the previous wave's stats
        //   ESC    – exit immediately

        if keys.just_pressed(KeyCode::Space) {
            state.prev_wave_client_count = state.total_clients;
            state.current_wave += 1;

            let manifest = env!("CARGO_MANIFEST_DIR");
            let cert_str = format!("{manifest}/examples/certs/cert.pem");
            let cert_path = Path::new(&cert_str);
            let ip: SocketAddr = BIND_ADDR.parse().unwrap();

            for _ in 0..CLIENTS_PER_WAVE {
                let mut client = match QuicClient::new_with_tls(&runtime, cert_path) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to create client: {e}");
                        continue;
                    }
                };
                let conn = client
                    .open_connection(Connect::new(ip).with_server_name("localhost"));
                commands.spawn(client).with_children(|p| {
                    p.spawn(conn);
                });
                state.total_clients += 1;
            }

            info!(
                "[Manual] +{CLIENTS_PER_WAVE} clients  →  {} total",
                state.total_clients
            );
        }

        if keys.just_pressed(KeyCode::Escape) {
            state.current_wave = MAX_WAVES;
            state.exit_deadline = Some(Instant::now());
        }
    } else {
        // ── Auto mode ──────────────────────────────────────────────────────

        if state.current_wave >= MAX_WAVES {
            return;
        }
        if Instant::now() < state.next_wave_at {
            return;
        }

        // Snapshot the client count *before* adding the new wave's clients.
        // This value will be used by `record_wave` to report the count that
        // was active during the sampling period that just ended.
        state.prev_wave_client_count = state.total_clients;

        state.current_wave += 1;
        state.next_wave_at = Instant::now() + Duration::from_secs_f64(RAMP_INTERVAL_SECS);

        let manifest = env!("CARGO_MANIFEST_DIR");
        let cert_str = format!("{manifest}/examples/certs/cert.pem");
        let cert_path = Path::new(&cert_str);
        let ip: SocketAddr = BIND_ADDR.parse().unwrap();

        let wave = state.current_wave;
        for _ in 0..CLIENTS_PER_WAVE {
            let mut client = match QuicClient::new_with_tls(&runtime, cert_path) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to create client: {e}");
                    continue;
                }
            };
            let conn =
                client.open_connection(Connect::new(ip).with_server_name("localhost"));
            commands.spawn(client).with_children(|p| {
                p.spawn(conn);
            });
            state.total_clients += 1;
        }

        info!(
            "[Wave {wave}/{MAX_WAVES}] +{CLIENTS_PER_WAVE} clients  →  {} total",
            state.total_clients
        );
    }
}

// ─── Opening streams ─────────────────────────────────────────────────────────

#[allow(clippy::type_complexity)]
fn open_streams(
    mut commands: Commands,
    connections: Query<
        (Entity, &mut QuicConnection),
        (Without<Children>, With<QuicClientMarker>),
    >,
) {
    for (entity, mut conn) in connections {
        match conn.open_bidrectional_stream() {
            Ok(bundle) => {
                commands.spawn((
                    bundle,
                    StressClientStream,
                    QuicClientMarker,
                    ChildOf(entity),
                ));
            }
            Err(e) => {
                warn!("open_streams: {e}");
            }
        }
    }
}

// ─── Client send (fixed tick) ────────────────────────────────────────────────

fn client_send(
    mut streams: Query<
        &mut QuicSendStream,
        (With<StressClientStream>, With<QuicClientMarker>),
    >,
) {
    for mut stream in &mut streams {
        if !stream.is_open() {
            continue;
        }
        let _ = stream.send(bytes::Bytes::from_static(PAYLOAD));
    }
}

// ─── Server receive (fixed tick) ─────────────────────────────────────────────
//
// Server-side streams are automatically accepted by SimpleServerAcceptorPlugin
// and tagged with QuicServerMarker (see server/acceptor.rs).

fn server_recv(
    mut streams: Query<&mut QuicReceiveStream, With<QuicServerMarker>>,
    mut state: ResMut<StressBench>,
) {
    state.bytes_this_frame = 0;

    for mut stream in &mut streams {
        if !stream.is_open() {
            continue;
        }

        let mut buf = Vec::new();
        let count = stream.recv_many(&mut buf, 512);
        let bytes: u64 = buf.iter().map(|p| p.payload.len() as u64).sum();

        state.bytes_this_frame += bytes;
        state.bytes_total += bytes;

        if count > 0 {
            stream.log_outstanding_errors();
        }
    }
}

// ─── Recording & reporting ───────────────────────────────────────────────────

fn record_wave(
    time: Res<Time>,
    mut state: ResMut<StressBench>,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame_times_ms.push(time.delta_secs_f64() * 1000.0);

    let wave = state.current_wave;
    let on_wave_boundary = wave > state.last_recorded_wave;

    // --- Record stats at each wave boundary ---
    if on_wave_boundary {
        // Always advance bookkeeping so this branch fires only once
        // per boundary, regardless of whether we actually record.
        state.last_recorded_wave = wave;

        let enough_samples = state.frame_times_ms.len() > 5;
        let has_clients = state.prev_wave_client_count > 0;

        if has_clients && enough_samples {
            let samples = std::mem::take(&mut state.frame_times_ms);
            let rx = std::mem::replace(&mut state.bytes_total, 0);
            let n = samples.len() as f64;
            let avg_ms = samples.iter().sum::<f64>() / n;
            let max_ms = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_fps = 1000.0 / max_ms;
            let avg_fps = 1000.0 / avg_ms;

            // A wave passes if the average frame time stays within the
            // 60 FPS budget.  Single-frame spikes are tolerated – only
            // sustained overload triggers a failure.
            let ok = avg_ms < FPS_THRESHOLD_MS;

            if !ok && state.first_fail_index.is_none() {
                state.first_fail_index = Some(state.results.len());
                warn!(
                    "⚠  Average FPS dropped below {} ({avg_fps:.1}) at {} clients",
                    state.prev_wave_client_count, FPS_THRESHOLD
                );
            }

            let result = WaveResult {
                client_count: state.prev_wave_client_count,
                max_ms,
                min_fps,
                avg_fps,
                rx_avg: rx / (samples.len().max(1) as u64),
                ok,
            };

            let status = if ok { "✓ OK" } else { "✗ FAIL" };
            info!(
                "[Run {:>2}]  clients={:>4}  avg={avg_ms:.2}ms  max={max_ms:.2}ms  \
                 avg_fps={avg_fps:.1}  min_fps={min_fps:.1}  rx={}/frame  [{status}]",
                state.results.len() + 1,
                result.client_count,
                fmt_bytes(result.rx_avg),
            );

            state.results.push(result);
        } else {
            // Baseline (0 clients) or too few samples – discard.
            state.frame_times_ms.clear();
            state.bytes_total = 0;
        }
    }

    // --- Once all waves have been spawned, set a deadline for the
    //     final measurement window so the last wave's data is captured. ---
    if wave >= MAX_WAVES && state.exit_deadline.is_none() {
        state.exit_deadline =
            Some(Instant::now() + Duration::from_secs_f64(RAMP_INTERVAL_SECS));
    }

    // --- Exit: first failing wave, or after the final-wave deadline. ---
    if state.first_fail_index.is_some() {
        if !state.report_printed {
            print_report(&state);
            state.report_printed = true;
        }
        exit.write(AppExit::Success);
    } else if let Some(deadline) = state.exit_deadline
        && Instant::now() >= deadline
    {
        // Final recording – dump whatever samples have accumulated
        // since the last wave boundary.
        if state.frame_times_ms.len() > 1 {
            let samples = std::mem::take(&mut state.frame_times_ms);
            let rx = std::mem::replace(&mut state.bytes_total, 0);
            let n = samples.len() as f64;
            let avg_ms = samples.iter().sum::<f64>() / n;
            let max_ms = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let avg_fps = 1000.0 / avg_ms;

            // Final wave always uses the *current* client count.
            let result = WaveResult {
                client_count: state.total_clients,
                max_ms,
                min_fps: 1000.0 / max_ms,
                avg_fps,
                rx_avg: rx / (samples.len().max(1) as u64),
                ok: avg_ms < FPS_THRESHOLD_MS,
            };

            let status = if result.ok { "✓ OK" } else { "✗ FAIL" };
            info!(
                "[Run {:>2}]  clients={:>4}  avg={avg_ms:.2}ms  max={max_ms:.2}ms  \
                     avg_fps={avg_fps:.1}  min_fps={:.1}  rx={}/frame  [{status}]",
                state.results.len() + 1,
                result.client_count,
                fmt_bytes(result.rx_avg),
                1000.0 / max_ms,
            );

            state.results.push(result);
        }
        if !state.report_printed {
            print_report(&state);
            state.report_printed = true;
        }
        exit.write(AppExit::Success);
    }
}

// ─── Final report ────────────────────────────────────────────────────────────

fn print_report(state: &StressBench) {
    let mut out = std::io::stderr().lock();

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "╔════════════════════════════════════════════════════╗"
    );
    let _ = writeln!(out, "║       QUIC CONCURRENCY STRESS TEST RESULTS        ║");
    let _ = writeln!(
        out,
        "╚════════════════════════════════════════════════════╝"
    );
    let elapsed = state.start.elapsed().as_secs_f64();
    let _ = writeln!(
        out,
        "  Duration: {elapsed:.1}s  |  Payload: {} B",
        PAYLOAD.len()
    );
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "  {:<5} {:<7} {:<8} {:<8} {:<8} {:<10} Status",
        "Run", "Clients", "Avg FPS", "Min FPS", "Max ms", "RX/frame"
    );
    let _ = writeln!(out, "  {}", "─".repeat(58));

    let mut max_ok = 0usize;

    for (i, r) in state.results.iter().enumerate() {
        let s = if r.ok {
            format!("✓ {} FPS", FPS_THRESHOLD)
        } else {
            "✗ DROPPED".to_owned()
        };
        let _ = writeln!(
            out,
            "  {i:<5} {:<7} {:<8.1} {:<8.1} {:<8.2} {:<10} {s}",
            r.client_count,
            r.avg_fps,
            r.min_fps,
            r.max_ms,
            fmt_bytes(r.rx_avg)
        );
        if r.ok {
            max_ok = r.client_count;
        }
    }

    let _ = writeln!(out, "  {}", "─".repeat(58));
    let _ = writeln!(out, "  Max concurrent clients at ≥60 FPS: {max_ok}");

    if let Some(first_fail_index) = state.first_fail_index {
        let fail_count = state.results[first_fail_index].client_count;
        let _ = writeln!(
            out,
            "  First failure at {fail_count} clients (average FPS dropped below 60)."
        );
    } else {
        let _ = writeln!(
            out,
            "  Completed all {MAX_WAVES} waves cleanly – no failure detected."
        );
        let _ = writeln!(out, "  Consider increasing MAX_WAVES to find the ceiling.");
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  Tip: run with --release for accurate results.");
    let _ = writeln!(out);
    let _ = out.flush();
}

fn fmt_bytes(b: u64) -> String {
    if b >= 1_048_576 {
        format!("{:.2} MB", b as f64 / 1_048_576.0)
    } else if b >= 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}
