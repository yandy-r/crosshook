/** Mock IPC handler signature — kept out of `../index` to avoid circular imports. */
export type Handler = (args: unknown) => unknown | Promise<unknown>;
