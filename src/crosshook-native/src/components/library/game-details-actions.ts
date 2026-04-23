/**
 * Orchestration for game-details quick actions: leave the detail surface before
 * navigation-heavy flows so focus and route stacks stay predictable.
 */

export function gameDetailsLaunchThenNavigate(
  profileName: string,
  launch: (name: string) => void | Promise<void>,
  exitDetailSurface: () => void
): void {
  exitDetailSurface();
  void launch(profileName);
}

export function gameDetailsEditThenNavigate(
  profileName: string,
  edit: (name: string) => void | Promise<void>,
  exitDetailSurface: () => void
): void {
  exitDetailSurface();
  void edit(profileName);
}
