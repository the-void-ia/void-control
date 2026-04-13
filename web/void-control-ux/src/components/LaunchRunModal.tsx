import { useMemo, useRef, useState } from 'react';

interface LaunchRunModalProps {
  open: boolean;
  isSubmitting: boolean;
  submitError: string | null;
  onClose: () => void;
  onSubmit: (params: { file: string; runId?: string; specText?: string }) => Promise<void>;
}

function looksLikeJson(text: string): boolean {
  const t = text.trim();
  return t.startsWith('{') || t.startsWith('[');
}

function validateSpecText(text: string): string[] {
  const trimmed = text.trim();
  if (!trimmed) return [];

  if (looksLikeJson(trimmed)) {
    try {
      const parsed = JSON.parse(trimmed) as Record<string, unknown>;
      const errors: string[] = [];
      const kind = typeof parsed.kind === 'string' ? parsed.kind : '';
      if (kind !== 'workflow' && kind !== 'pipeline') {
        errors.push("`kind` must be `workflow` or `pipeline`.");
      }
      if (typeof parsed.name !== 'string' || parsed.name.trim().length === 0) errors.push('`name` is required.');
      if (kind === 'workflow') {
        if (typeof parsed.workflow !== 'object' || parsed.workflow === null) errors.push('`workflow` section is required.');
        const workflow = parsed.workflow as Record<string, unknown> | undefined;
        if (!workflow || !Array.isArray(workflow.steps)) errors.push("Missing `steps:` list under workflow.");
      }
      if (kind === 'pipeline') {
        if (typeof parsed.pipeline !== 'object' || parsed.pipeline === null) errors.push('`pipeline` section is required.');
        const pipeline = parsed.pipeline as Record<string, unknown> | undefined;
        if (!pipeline || !Array.isArray(pipeline.boxes)) errors.push("Missing `boxes:` list under pipeline.");
        if (!pipeline || !Array.isArray(pipeline.stages)) errors.push("Missing `stages:` list under pipeline.");
      }
      return errors;
    } catch {
      return ['Invalid JSON format.'];
    }
  }

  const errors: string[] = [];
  const kindMatch = /^\s*kind\s*:\s*([a-zA-Z0-9_-]+)\s*$/m.exec(trimmed);
  const kind = kindMatch?.[1]?.toLowerCase() ?? '';
  const modeMatch = /^\s*mode\s*:\s*([a-zA-Z0-9_-]+)\s*$/m.exec(trimmed);
  const hasGoal = /^\s*goal\s*:\s*/m.test(trimmed);

  if (modeMatch && hasGoal) {
    return [];
  }

  if (!/^\s*api_version\s*:\s*/m.test(trimmed)) errors.push('Missing `api_version:`.');
  if (!kindMatch) {
    errors.push('Missing `kind:` for runtime specs or `mode:` + `goal:` for orchestration specs.');
  } else if (kind !== 'workflow' && kind !== 'pipeline') {
    errors.push("`kind` must be `workflow` or `pipeline`.");
  }
  if (!/^\s*name\s*:\s*/m.test(trimmed)) errors.push('Missing `name:`.');
  if (!/^\s*sandbox\s*:\s*/m.test(trimmed)) errors.push('Missing `sandbox:` section.');
  if (kind === 'workflow') {
    if (!/^\s*workflow\s*:\s*/m.test(trimmed)) errors.push('Missing `workflow:` section.');
    if (!/^\s*steps\s*:\s*/m.test(trimmed)) errors.push('Missing `steps:` list under workflow.');
  }
  if (kind === 'pipeline') {
    if (!/^\s*pipeline\s*:\s*/m.test(trimmed)) errors.push('Missing `pipeline:` section.');
    if (!/^\s*boxes\s*:\s*/m.test(trimmed)) errors.push('Missing `boxes:` list under pipeline.');
    if (!/^\s*stages\s*:\s*/m.test(trimmed)) errors.push('Missing `stages:` list under pipeline.');
  }
  return errors;
}

export function LaunchRunModal({
  open,
  isSubmitting,
  submitError,
  onClose,
  onSubmit
}: LaunchRunModalProps) {
  const [filePath, setFilePath] = useState('/tmp/void-control-run.yaml');
  const [runId, setRunId] = useState(`ui-${Date.now()}`);
  const [specText, setSpecText] = useState('');
  const [uploadedName, setUploadedName] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement | null>(null);

  const validationErrors = useMemo(() => validateSpecText(specText), [specText]);

  if (!open) return null;

  const onPickFile = async (file?: File) => {
    if (!file) return;
    const text = await file.text();
    setSpecText(text);
    setUploadedName(file.name);
    if (!filePath || filePath.trim().length === 0 || filePath === '/tmp/void-control-run.yaml') {
      const safeName = file.name.replace(/[^a-zA-Z0-9._-]/g, '_');
      setFilePath(`/tmp/${safeName}`);
    }
  };

  const submit = async () => {
    const trimmedPath = filePath.trim();
    if (!trimmedPath) return;
    if (validationErrors.length > 0) return;
    await onSubmit({
      file: trimmedPath,
      runId: runId.trim().length > 0 ? runId.trim() : undefined,
      specText: specText.trim().length > 0 ? specText : undefined
    });
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="launch-modal" onClick={(e) => e.stopPropagation()}>
        <div className="launch-modal-head">
          <h3>Launch Spec</h3>
          <button type="button" className="inspector-btn" onClick={onClose}>Close</button>
        </div>

        <div className="launch-form-grid">
          <label>
            <span>Spec File Path (daemon host)</span>
            <input
              value={filePath}
              onChange={(e) => setFilePath(e.target.value)}
              placeholder="/tmp/spec.yaml"
            />
          </label>
          <label>
            <span>Run ID (optional)</span>
            <input
              value={runId}
              onChange={(e) => setRunId(e.target.value)}
              placeholder="ui-demo-1"
            />
          </label>
        </div>

        <div className="launch-upload-row">
          <button type="button" className="launch-upload-btn" onClick={() => fileRef.current?.click()}>
            Upload YAML/JSON
          </button>
          {uploadedName && <span className="launch-upload-name">{uploadedName}</span>}
          <input
            ref={fileRef}
            type="file"
            accept=".yaml,.yml,.json,text/yaml,application/json"
            style={{ display: 'none' }}
            onChange={(e) => void onPickFile(e.target.files?.[0])}
          />
        </div>

        <label className="launch-spec-label">
          <span>Spec Content (runtime or orchestration YAML)</span>
          <textarea
            value={specText}
            onChange={(e) => setSpecText(e.target.value)}
            placeholder="Paste workflow, pipeline, or swarm/supervision YAML here..."
          />
        </label>

        {validationErrors.length > 0 && (
          <div className="launch-error-list">
            {validationErrors.map((err) => <div key={err}>- {err}</div>)}
          </div>
        )}

        {submitError && <div className="launch-error-list">- {submitError}</div>}

        <div className="launch-note">
          If spec content is provided, the UI will try orchestration launch first and fall back to runtime launch for raw workflow/pipeline specs.
          If spec content is empty, launch falls back to file-path mode and the path must exist on the daemon host.
        </div>

        <div className="launch-actions">
          <button type="button" className="inspector-btn" onClick={onClose}>Cancel</button>
          <button type="button" className="launch-primary-btn" onClick={() => void submit()} disabled={isSubmitting || !filePath.trim() || validationErrors.length > 0}>
            {isSubmitting ? 'Launching...' : 'Launch'}
          </button>
        </div>
      </div>
    </div>
  );
}
