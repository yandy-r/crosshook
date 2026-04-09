/**
 * Thin orchestration for game-details quick actions: close the modal before
 * navigation-heavy flows so focus restoration and route stacks stay predictable.
 */

export function gameDetailsLaunchThenNavigate(
  profileName: string,
  launch: (name: string) => void | Promise<void>,
  closeModal: () => void
): void {
  closeModal();
  void launch(profileName);
}

export function gameDetailsEditThenNavigate(
  profileName: string,
  edit: (name: string) => void | Promise<void>,
  closeModal: () => void
): void {
  closeModal();
  void edit(profileName);
}
