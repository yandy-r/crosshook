interface WizardStepperProps {
  labels: readonly string[];
  currentStep: number;
}

export function WizardStepper({ labels, currentStep }: WizardStepperProps) {
  return (
    <div className="crosshook-community-import-wizard__stepper">
      {labels.map((label, index) => (
        <div
          key={label}
          className={[
            'crosshook-community-import-wizard__step',
            index === currentStep ? 'crosshook-community-import-wizard__step--active' : '',
          ].join(' ')}
        >
          <span className="crosshook-community-import-wizard__step-index">{index + 1}</span>
          <span>{label}</span>
        </div>
      ))}
    </div>
  );
}

export default WizardStepper;
