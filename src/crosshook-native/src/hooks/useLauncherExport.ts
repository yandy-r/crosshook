import { useCallback, useEffect, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import type {
  GameProfile,
  GamescopeConfig,
  LaunchMethod,
  LauncherDeleteResult,
  LauncherInfo,
  TrainerLoadingMode,
} from '../types';

export interface SteamExternalLauncherExportRequest {
  method: string;
  launcher_name: string;
  trainer_path: string;
  trainer_loading_mode: TrainerLoadingMode;
  launcher_icon_path: string;
  prefix_path: string;
  proton_path: string;
  steam_app_id: string;
  steam_client_install_path: string;
  target_home_path: string;
  profile_name?: string;
  network_isolation: boolean;
  gamescope?: GamescopeConfig;
}

export interface SteamExternalLauncherExportResult {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
}

export interface UseLauncherExportOptions {
  request: SteamExternalLauncherExportRequest;
  profile: GameProfile;
  steamClientInstallPath: string;
  targetHomePath: string;
  pendingReExport?: boolean;
  onReExportHandled?: () => void;
}

function collectDeleteWarnings(result: LauncherDeleteResult): string[] {
  return [result.script_skipped_reason, result.desktop_entry_skipped_reason].filter(
    (value): value is string => typeof value === 'string' && value.trim().length > 0
  );
}

export function useLauncherExport({
  request,
  profile,
  steamClientInstallPath,
  targetHomePath,
  pendingReExport,
  onReExportHandled,
}: UseLauncherExportOptions) {
  const [isExporting, setIsExporting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [result, setResult] = useState<SteamExternalLauncherExportResult | null>(null);
  const [launcherStatus, setLauncherStatus] = useState<LauncherInfo | null>(null);
  const [deleteConfirming, setDeleteConfirming] = useState(false);
  const deleteTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [showLauncherPreview, setShowLauncherPreview] = useState(false);
  const [previewScriptContent, setPreviewScriptContent] = useState('');
  const [previewDesktopContent, setPreviewDesktopContent] = useState('');
  const [previewLoading, setPreviewLoading] = useState(false);

  const refreshLauncherStatus = useCallback(async () => {
    try {
      const info = await callCommand<LauncherInfo>('check_launcher_exists', { request });
      setLauncherStatus(info);
    } catch (error) {
      console.error('Failed to refresh launcher status.', error);
      setErrorMessage(`Failed to check launcher status: ${error instanceof Error ? error.message : String(error)}`);
      setLauncherStatus(null);
    }
  }, [request]);

  useEffect(() => {
    void refreshLauncherStatus();
  }, [refreshLauncherStatus]);

  useEffect(() => {
    return () => {
      if (deleteTimeoutRef.current !== null) {
        clearTimeout(deleteTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!pendingReExport) return;

    const timer = setTimeout(() => {
      void (async () => {
        try {
          await callCommand<void>('validate_launcher_export', { request });
          await callCommand<SteamExternalLauncherExportResult>('export_launchers', { request });
          void refreshLauncherStatus();
        } catch {
          // Silent — user can manually re-export if auto-export fails
        } finally {
          onReExportHandled?.();
        }
      })();
    }, 150);

    return () => clearTimeout(timer);
  }, [pendingReExport, request, refreshLauncherStatus, onReExportHandled]);

  const exportLauncher = useCallback(async () => {
    setIsExporting(true);
    setErrorMessage(null);
    setStatusMessage(null);
    setResult(null);

    try {
      await callCommand<void>('validate_launcher_export', { request });
      const exported = await callCommand<SteamExternalLauncherExportResult>('export_launchers', { request });
      setResult(exported);
      setStatusMessage('Launcher export completed.');
      void refreshLauncherStatus();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsExporting(false);
    }
  }, [request, refreshLauncherStatus]);

  const previewLauncher = useCallback(async () => {
    setPreviewLoading(true);
    setErrorMessage(null);
    try {
      const [script, desktop] = await Promise.all([
        callCommand<string>('preview_launcher_script', { request }),
        callCommand<string>('preview_launcher_desktop', { request }),
      ]);
      setPreviewScriptContent(script);
      setPreviewDesktopContent(desktop);
      setShowLauncherPreview(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setPreviewLoading(false);
    }
  }, [request]);

  const deleteLauncher = useCallback(async () => {
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const deleteResult = await callCommand<LauncherDeleteResult>('delete_launcher', {
        displayName: profile.steam?.launcher?.display_name || '',
        steamAppId: profile.steam?.app_id || '',
        trainerPath: profile.trainer?.path || '',
        targetHomePath: targetHomePath || '',
        steamClientInstallPath: steamClientInstallPath || '',
      });
      const warnings = collectDeleteWarnings(deleteResult);
      const deletedAny = deleteResult.script_deleted || deleteResult.desktop_entry_deleted;

      if (deletedAny && warnings.length === 0) {
        setStatusMessage('Launcher deleted.');
      } else if (deletedAny) {
        setStatusMessage(`Launcher deleted with warnings: ${warnings.join(' ')}`);
      } else if (warnings.length > 0) {
        setErrorMessage(`Launcher was not deleted: ${warnings.join(' ')}`);
      } else {
        setStatusMessage('Launcher files were already absent.');
      }

      void refreshLauncherStatus();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }, [
    profile.steam?.app_id,
    profile.steam?.launcher?.display_name,
    profile.trainer?.path,
    refreshLauncherStatus,
    steamClientInstallPath,
    targetHomePath,
  ]);

  const handleDeleteClick = useCallback(() => {
    if (deleteConfirming) {
      if (deleteTimeoutRef.current !== null) {
        clearTimeout(deleteTimeoutRef.current);
        deleteTimeoutRef.current = null;
      }
      setDeleteConfirming(false);
      void deleteLauncher();
    } else {
      setDeleteConfirming(true);
      deleteTimeoutRef.current = setTimeout(() => {
        setDeleteConfirming(false);
        deleteTimeoutRef.current = null;
      }, 3000);
    }
  }, [deleteConfirming, deleteLauncher]);

  const handleDeleteBlur = useCallback(() => {
    if (deleteConfirming) {
      if (deleteTimeoutRef.current !== null) {
        clearTimeout(deleteTimeoutRef.current);
        deleteTimeoutRef.current = null;
      }
      setDeleteConfirming(false);
    }
  }, [deleteConfirming]);

  const clearExportFeedback = useCallback(() => {
    setErrorMessage(null);
    setStatusMessage(null);
    setResult(null);
  }, []);

  return {
    launcherStatus,
    errorMessage,
    setErrorMessage,
    statusMessage,
    result,
    isExporting,
    previewLoading,
    previewScriptContent,
    previewDesktopContent,
    showLauncherPreview,
    setShowLauncherPreview,
    deleteConfirming,
    deleteTimeoutRef,
    refreshLauncherStatus,
    exportLauncher,
    previewLauncher,
    deleteLauncher,
    handleDeleteClick,
    handleDeleteBlur,
    clearExportFeedback,
  };
}
