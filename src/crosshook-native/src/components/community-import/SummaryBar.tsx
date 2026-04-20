interface SummaryBarProps {
  autoResolvedCount: number;
  hasManualAdjustments: boolean;
  unresolvedCount: number;
}

export function SummaryBar({ autoResolvedCount, hasManualAdjustments, unresolvedCount }: SummaryBarProps) {
  return (
    <div className="crosshook-community-import-wizard__summary">
      <span>Auto-resolved: {autoResolvedCount}</span>
      <span>Manual edits: {hasManualAdjustments ? 'Yes' : 'No'}</span>
      <span>Unresolved required fields: {unresolvedCount}</span>
    </div>
  );
}

export default SummaryBar;
