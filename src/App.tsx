import { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Mic,
  Headphones,
  Volume2,
  RefreshCw,
  Play,
  Square,
  CheckCircle2,
  AlertTriangle,
  Layers,
  HelpCircle,
  Sparkles,
} from 'lucide-react';

interface DeviceEntry {
  id: string;
  name: string;
  is_default: boolean;
  is_virtual: boolean;
}

interface DeviceLists {
  microphones: DeviceEntry[];
  virtual_outputs: DeviceEntry[];
  cable_installed: boolean;
}

interface AppEntry {
  pid: number;
  name: string;
  exe_name: string;
  is_audio_active: boolean;
}

export default function App() {
  const [deviceLists, setDeviceLists] = useState<DeviceLists>({
    microphones: [],
    virtual_outputs: [],
    cable_installed: false,
  });

  const [apps, setApps] = useState<AppEntry[]>([]);
  const [selectedMic, setSelectedMic] = useState<string>('');
  const [selectedVirtualOutput, setSelectedVirtualOutput] = useState<string>('');
  const [sourceMode, setSourceMode] = useState<'app' | 'global'>('app');
  const [selectedPid, setSelectedPid] = useState<number | ''>('');
  const includeTree = true;
  const [appSearch, setAppSearch] = useState<string>('');

  const [micVolume, setMicVolume] = useState<number>(100);
  const [appVolume, setAppVolume] = useState<number>(100);
  const [appNormalize, setAppNormalize] = useState<boolean>(true);

  const [isStreaming, setIsStreaming] = useState<boolean>(false);
  const [isRefreshing, setIsRefreshing] = useState<boolean>(false);
  const [statusError, setStatusError] = useState<string | null>(null);

  // Initial load
  useEffect(() => {
    loadDevices();
    loadApps();
  }, []);

  const loadDevices = async () => {
    try {
      const lists = await invoke<DeviceLists>('get_devices');
      setDeviceLists(lists);

      if (lists.microphones.length > 0) {
        const defaultMic = lists.microphones.find((m) => m.is_default);
        setSelectedMic(defaultMic ? defaultMic.id : lists.microphones[0].id);
      }

      if (lists.virtual_outputs.length > 0) {
        // Find VB-Cable or virtual first
        const cable = lists.virtual_outputs.find((v) => v.is_virtual);
        setSelectedVirtualOutput(cable ? cable.id : lists.virtual_outputs[0].id);
      }
    } catch (err) {
      console.error('Failed to load devices:', err);
      setStatusError(String(err));
    }
  };

  const loadApps = async () => {
    setIsRefreshing(true);
    try {
      const list = await invoke<AppEntry[]>('get_running_apps');
      setApps(list);
      // Auto-select first active audio process if not already set
      if (list.length > 0) {
        const activeApp = list.find((a) => a.is_audio_active);
        setSelectedPid((prev) => (prev !== '' && list.some(a => a.pid === prev) ? prev : activeApp ? activeApp.pid : list[0].pid));
      }
    } catch (err) {
      console.error('Failed to load apps:', err);
    } finally {
      setTimeout(() => setIsRefreshing(false), 300);
    }
  };

  const filteredApps = useMemo(() => {
    if (!appSearch.trim()) return apps;
    const q = appSearch.toLowerCase();
    return apps.filter(
      (a) =>
        a.name.toLowerCase().includes(q) ||
        a.pid.toString().includes(q) ||
        a.exe_name.toLowerCase().includes(q)
    );
  }, [apps, appSearch]);

  const handleMicVolumeChange = (vol: number) => {
    setMicVolume(vol);
    invoke('set_mic_volume', { volume: vol / 100 }).catch((e) =>
      console.error('Failed to set mic volume:', e)
    );
  };

  const handleAppVolumeChange = (vol: number) => {
    setAppVolume(vol);
    invoke('set_app_volume', { volume: vol / 100 }).catch((e) =>
      console.error('Failed to set app volume:', e)
    );
  };

  const handleToggleNormalize = (enabled: boolean) => {
    setAppNormalize(enabled);
    invoke('set_app_normalize', { enabled }).catch((e) =>
      console.error('Failed to set app normalize:', e)
    );
  };

  const handleToggleStreaming = async () => {
    setStatusError(null);
    if (isStreaming) {
      try {
        await invoke('stop_streaming');
        setIsStreaming(false);
      } catch (err) {
        console.error('Failed to stop streaming:', err);
        setStatusError(String(err));
      }
    } else {
      if (sourceMode === 'app' && selectedPid === '') {
        setStatusError('Please select a running application to capture.');
        return;
      }

      const source =
        sourceMode === 'app'
          ? { type: 'Application', pid: Number(selectedPid), include_tree: includeTree }
          : { type: 'GlobalHeadphones' };

      const config = {
        mic_device_id: selectedMic,
        virtual_output_id: selectedVirtualOutput,
        source,
      };

      try {
        await invoke('start_streaming', { config });
        setIsStreaming(true);
      } catch (err) {
        console.error('Failed to start streaming:', err);
        setStatusError(String(err));
      }
    }
  };

  return (
    <div className="app-container">
      {/* Header */}
      <div className="app-header">
        <div className="app-title">
          <Volume2 size={20} />
          <span>Virtual Mic & Audio Share</span>
        </div>
        <div className={`live-badge ${isStreaming ? 'active' : 'idle'}`}>
          {isStreaming && <div className="pulse-dot" />}
          {isStreaming ? 'STREAMING' : 'IDLE'}
        </div>
      </div>

      {/* Virtual Cable Status Banner */}
      {deviceLists.cable_installed ? (
        <div className="cable-banner ready">
          <CheckCircle2 size={16} />
          <span>Virtual Cable Ready: Outputting to "CABLE Input"</span>
        </div>
      ) : (
        <div className="cable-banner warning">
          <AlertTriangle size={16} />
          <div>
            VB-Cable not detected.{' '}
            <a
              href="https://vb-audio.com/Cable/"
              target="_blank"
              rel="noreferrer"
            >
              Download VB-Audio Cable
            </a>{' '}
            for full virtual mic support.
          </div>
        </div>
      )}

      {/* Quick Setup Guide for Destination Apps */}
      <div className="setup-guide">
        <HelpCircle size={16} className="setup-guide-icon" />
        <div>
          <div className="setup-guide-title">How other apps hear this mixed audio:</div>
          <div>
            In <strong>Discord, Zoom, OBS, or Games</strong>, set your <strong>Input Device (Microphone)</strong> to <strong>"CABLE Output (VB-Audio Virtual Cable)"</strong>.
          </div>
        </div>
      </div>

      {/* Input Device Card */}
      <div className="card">
        <div className="card-title">
          <Mic size={16} />
          <span>Physical Microphone Input</span>
        </div>

        <div className="control-group">
          <label className="control-label">
            <span>Select Microphone</span>
          </label>
          <select
            value={selectedMic}
            onChange={(e) => setSelectedMic(e.target.value)}
            disabled={isStreaming}
          >
            {deviceLists.microphones.map((mic) => (
              <option key={mic.id} value={mic.id}>
                {mic.name} {mic.is_default ? '(Default)' : ''}
              </option>
            ))}
          </select>
        </div>

        <div className="control-group">
          <div className="control-label">
            <span>Microphone Gain (double-click to reset)</span>
            <span className="volume-val">{micVolume}%</span>
          </div>
          <input
            type="range"
            min={0}
            max={200}
            step={1}
            value={micVolume}
            onChange={(e) => handleMicVolumeChange(Number(e.target.value))}
            onDoubleClick={() => handleMicVolumeChange(100)}
            title="Double-click to reset to 100%"
          />
        </div>
      </div>

      {/* Audio Source Card */}
      <div className="card">
        <div className="card-title">
          <Headphones size={16} />
          <span>Audio Source (Mixer Stream)</span>
        </div>

        <div className="segmented-control">
          <button
            type="button"
            className={`segment-btn ${sourceMode === 'app' ? 'active' : ''}`}
            onClick={() => !isStreaming && setSourceMode('app')}
            disabled={isStreaming}
          >
            <Layers size={13} />
            Specific Application
          </button>
          <button
            type="button"
            className={`segment-btn ${sourceMode === 'global' ? 'active' : ''}`}
            onClick={() => !isStreaming && setSourceMode('global')}
            disabled={isStreaming}
          >
            <Headphones size={13} />
            All System Audio (Global)
          </button>
        </div>

        {sourceMode === 'app' && (
          <div className="control-group">
            <div className="control-label">
              <span>Target Application</span>
              <span>{filteredApps.length} apps available</span>
            </div>

            <div className="app-select-row">
              <input
                type="text"
                placeholder="Search apps (e.g. Discord, Spotify, Chrome)..."
                value={appSearch}
                onChange={(e) => setAppSearch(e.target.value)}
                disabled={isStreaming}
                style={{ flex: 1 }}
              />
              <button
                type="button"
                className={`refresh-btn ${isRefreshing ? 'spinning' : ''}`}
                onClick={loadApps}
                disabled={isStreaming || isRefreshing}
                title="Refresh active audio applications"
              >
                <RefreshCw size={14} />
              </button>
            </div>

            <select
              value={selectedPid}
              onChange={(e) => setSelectedPid(e.target.value ? Number(e.target.value) : '')}
              disabled={isStreaming}
            >
              {filteredApps.length === 0 ? (
                <option value="">No matching applications found</option>
              ) : (
                filteredApps.map((app) => (
                  <option key={app.pid} value={app.pid}>
                    {app.name} {app.is_audio_active ? '🔊 [Active Audio]' : ''}
                  </option>
                ))
              )}
            </select>

            <div className="app-subtext">
              <Sparkles size={12} />
              <span>Captures entire application & child subprocesses</span>
            </div>
          </div>
        )}

        <div className="control-group">
          <div className="control-label">
            <span>Streamed Audio Gain (double-click to reset)</span>
            <span className="volume-val">{appVolume}%</span>
          </div>
          <input
            type="range"
            min={0}
            max={200}
            step={1}
            value={appVolume}
            onChange={(e) => handleAppVolumeChange(Number(e.target.value))}
            onDoubleClick={() => handleAppVolumeChange(100)}
            title="Double-click to reset to 100%"
          />

          <label className="normalize-toggle">
            <input
              type="checkbox"
              checked={appNormalize}
              onChange={(e) => handleToggleNormalize(e.target.checked)}
            />
            <span>Auto-Normalize Audio (Stable loudness, prevents dipping)</span>
          </label>
        </div>
      </div>

      {statusError && <div className="error-banner">{statusError}</div>}

      {/* Master Action Bar */}
      <div className="master-bar">
        <button
          type="button"
          className={`master-btn ${isStreaming ? 'stop' : 'start'}`}
          onClick={handleToggleStreaming}
        >
          {isStreaming ? (
            <>
              <Square size={16} fill="currentColor" />
              <span>Stop Audio Share</span>
            </>
          ) : (
            <>
              <Play size={16} fill="currentColor" />
              <span>Start Audio Share</span>
            </>
          )}
        </button>
      </div>
    </div>
  );
}
