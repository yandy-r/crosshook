export function HostDelegationBanner() {
  return (
    <div className="crosshook-info-banner" role="note">
      <strong>Host tools are always detected on the host system.</strong> CrossHook checks the host install in both
      native and Flatpak environments. If you are using the Flatpak build, required tools still need to be installed on
      the host rather than bundled inside the sandbox.
    </div>
  );
}

export default HostDelegationBanner;
