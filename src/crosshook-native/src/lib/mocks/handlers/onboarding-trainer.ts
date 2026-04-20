import type { TrainerGuidanceContent } from '../../../types/onboarding';

export function buildTrainerGuidanceContent(): TrainerGuidanceContent {
  return {
    loading_modes: [
      {
        id: 'source_directory',
        title: 'Source Directory',
        description: 'Proton reads the trainer directly from its downloaded location. The trainer stays in place.',
        when_to_use: 'Use when the trainer runs standalone without extra DLLs or support files.',
        examples: ['FLiNG single-file .exe trainers'],
      },
      {
        id: 'copy_to_prefix',
        title: 'Copy to Prefix',
        description: "CrossHook copies the trainer and support files into the WINE prefix's C:\\ drive before launch.",
        when_to_use: 'Use when the trainer bundles DLLs or support files that must be present in the WINE prefix.',
        examples: ['FLiNG trainers that bundle DLLs', 'Trainers with companion .ini or .dat files'],
      },
    ],
    trainer_sources: [
      {
        id: 'fling',
        title: 'FLiNG Trainers',
        description: 'FLiNG standalone .exe trainers — free, no account required. Primary recommendation.',
        when_to_use: 'Primary recommendation — no account needed, direct .exe download.',
        examples: ['flingtrainer.com standalone executables'],
      },
      {
        id: 'wemod',
        title: 'WeMod',
        description:
          'WeMod extracted trainers — requires a WeMod account and the WeMod desktop app installed under WINE.',
        when_to_use: 'Use only if WeMod is already set up under WINE. See wemod-launcher for setup instructions.',
        examples: ['WeMod extracted trainer DLLs'],
      },
    ],
    verification_steps: [
      'Verify the trainer .exe file exists at the configured path.',
      "Confirm the game version matches the trainer's target version.",
      'For Copy to Prefix mode: ensure companion DLLs and support files are in the same directory.',
      'Launch the game at least once to initialize the WINE prefix before using trainers.',
    ],
  };
}
