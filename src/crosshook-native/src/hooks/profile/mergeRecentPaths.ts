export function mergeRecentPaths(currentPaths: string[], nextPath: string): string[] {
  const trimmed = nextPath.trim();
  if (!trimmed) {
    return currentPaths;
  }

  return [trimmed, ...currentPaths.filter((path) => path !== trimmed)].slice(0, 10);
}
