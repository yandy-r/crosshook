/**
 * Library card interaction contract:
 * - `onOpenDetails` fires from the card details hit target (not footer actions).
 * - Footer buttons call `stopPropagation()` so Launch / Favorite / Edit stay isolated
 *   from the details-open control (ordering: footer handlers run without bubbling to the body layer).
 */
export type LibraryOpenDetailsHandler = (profileName: string) => void;
