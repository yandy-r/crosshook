/**
 * Short tooltip copy for host readiness catalog tools: purpose + what you lose if missing.
 * Keys match `tool_id` in `default_host_readiness_catalog.toml`.
 */
const HOST_TOOL_TOOLTIPS: Record<string, string> = {
  umu_run:
    'umu-launcher improves Proton runtime bootstrapping for non-Steam launches. Without it: CrossHook falls back to Proton directly and may miss umu-specific fixes.',
  gamescope:
    'Gamescope is a micro-compositor for nested sessions, scaling, and HDR. Without it: no host gamescope for Gamescope session features CrossHook checks.',
  mangohud:
    'MangoHud shows an on-screen FPS and frame-time overlay. Without it: no MangoHud overlay on host-run commands.',
  gamemode:
    'GameMode requests CPU governor and I/O tweaks while games run. Without it: no automatic GameMode tuning from the host binary.',
  game_performance:
    'CachyOS utility for gaming performance tweaks. Without it: CachyOS game-performance integration will not run. CachyOS-specific utility; available on CachyOS repositories.',
  winetricks:
    'Winetricks installs Windows DLLs/components into Wine prefixes. Without it: harder to fix prefix dependencies without manual steps.',
  protontricks:
    'Protontricks applies winetricks to Proton prefixes for Steam games. Without it: no protontricks CLI for Proton prefix fixes.',
  wine_wayland:
    'Host Wine enables Wayland-related experiments with PROTON_ENABLE_WAYLAND. Without it: host Wine/Wayland checks CrossHook uses cannot succeed.',
};

/** Tooltip for a host catalog tool; falls back to a generic message for unknown IDs. */
export function getHostToolTooltipContent(toolId: string): string {
  const text = HOST_TOOL_TOOLTIPS[toolId];
  if (text !== undefined) {
    return text;
  }
  return 'Host tool used during readiness checks. Without it: related host features CrossHook probes may be unavailable.';
}
