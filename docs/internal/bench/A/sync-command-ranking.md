# E1 Classic synchronous IPC command ranking

This is the M0-03 input from [YAN-795](https://linear.app/itcn/issue/YAN-795). The native timing build used [variant A](summary.md) plus the local, throwaway timing-shim commit `af36f40917460d75b9a54765ff7ddc7f76411498`. The shim wraps Tauri invoke dispatch and emits command name, thread ID, and monotonic elapsed time with `CROSSHOOK_BENCH=1`. Both captures ran on 2026-11-25 with seed 42, 500 profiles, and a 50,000-line replay.

The two sessions yielded 163 synchronous samples across 28 commands, all on `ThreadId(1)`. The second session adds Sync Taps using a local Git fixture and a successful diagnostic export. Readiness samples came from startup. The export is already async in the tag and belongs to the 113 async dispatch records excluded from the table. The measured interval includes synchronous Tauri dispatch and response overhead. No scheduler trace was collected.

The table is sorted by maximum observed duration. p50 is the median and p95 uses nearest rank. A single sample gives the same value in all three columns. Classifications come from the command paths in the tag. Disk includes filesystem and SQLite operations, some of which hit caches. Subprocess covers commands that invoke Git or host probes and can also touch disk. A local Git sync does not measure remote network latency. [Synchronous samples (CSV)](E1/sync-command-samples.csv)

| Command                                     | Work       | Samples | p50 (ms) | p95 (ms) | Max (ms) |
| ------------------------------------------- | ---------- | ------: | -------: | -------: | -------: |
| `community_sync`                            | Subprocess |       1 |  192.163 |  192.163 |  192.163 |
| `batch_validate_profiles`                   | Disk       |       4 |  102.635 |  129.620 |  129.620 |
| `check_generalized_readiness`               | Subprocess |       2 |    6.751 |   13.122 |   13.122 |
| `profile_list_summaries`                    | Disk       |      14 |    7.048 |   12.837 |   12.837 |
| `community_list_profiles`                   | Disk       |       3 |    0.092 |   11.651 |   11.651 |
| `list_launchers`                            | Disk       |       1 |    7.296 |    7.296 |    7.296 |
| `check_game_running`                        | Disk       |       8 |    4.300 |    6.955 |    6.955 |
| `discovery_catalog`                         | Disk       |       2 |    3.277 |    4.157 |    4.157 |
| `community_list_indexed_profiles`           | Disk       |       1 |    3.875 |    3.875 |    3.875 |
| `get_cached_offline_readiness_snapshots`    | Disk       |       9 |    0.911 |    1.580 |    1.580 |
| `get_capabilities`                          | Disk       |       4 |    0.354 |    0.779 |    0.779 |
| `get_cached_health_snapshots`               | Disk       |       2 |    0.601 |    0.777 |    0.777 |
| `profile_list`                              | Disk       |      16 |    0.187 |    0.434 |    0.434 |
| `list_launch_history_for_profile`           | Disk       |       2 |    0.271 |    0.369 |    0.369 |
| `list_running_profiles`                     | In memory  |      59 |    0.020 |    0.044 |    0.180 |
| `profile_set_favorite`                      | Disk       |       2 |    0.144 |    0.156 |    0.156 |
| `profile_load`                              | Disk       |       6 |    0.142 |    0.155 |    0.155 |
| `profile_list_favorites`                    | Disk       |       6 |    0.105 |    0.147 |    0.147 |
| `detect_protontricks_binary`                | Disk       |       1 |    0.127 |    0.127 |    0.127 |
| `collection_list`                           | Disk       |       2 |    0.101 |    0.122 |    0.122 |
| `get_cached_host_readiness_snapshot`        | Disk       |       4 |    0.075 |    0.111 |    0.111 |
| `profile_list_bundled_optimization_presets` | Disk       |       2 |    0.067 |    0.089 |    0.089 |
| `metadata_store_status`                     | In memory  |       2 |    0.038 |    0.057 |    0.057 |
| `list_proton_installs`                      | Disk       |       3 |    0.027 |    0.049 |    0.049 |
| `check_launcher_exists`                     | Disk       |       1 |    0.030 |    0.030 |    0.030 |
| `protonup_list_providers`                   | In memory  |       2 |    0.019 |    0.022 |    0.022 |
| `default_steam_client_install_path`         | Disk       |       2 |    0.019 |    0.021 |    0.021 |
| `get_trainer_type_catalog`                  | In memory  |       2 |    0.014 |    0.017 |    0.017 |

Community sync and batch profile validation had the largest observed synchronous dispatch times. Check these paths first when verifying the current M0-03 async boundaries. The table describes the release tag and does not establish which handlers still block on current main.
