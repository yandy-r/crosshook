export const MAIN_CONTENT_ID = 'crosshook-main-content';

/**
 * First tab stop of the document. Programmatic focus because hash navigation is
 * unreliable in the no-router Tabs SPA under WebKitGTK.
 */
export function SkipToContentLink() {
  return (
    <a
      href={`#${MAIN_CONTENT_ID}`}
      className="crosshook-skip-link"
      onClick={(event) => {
        event.preventDefault();
        document.getElementById(MAIN_CONTENT_ID)?.focus();
      }}
    >
      Skip to main content
    </a>
  );
}
