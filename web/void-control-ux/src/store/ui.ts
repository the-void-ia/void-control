import { create } from 'zustand';
import type { SelectedNodeType } from '../lib/selectors';

interface UiState {
  selectedRunId: string | null;
  selectedNodeId: string | null;
  selectedNodeType: SelectedNodeType;
  isSelectionPinned: boolean;
  lastSeenEventByRun: Record<string, string | undefined>;
  setSelectedRunId: (runId: string | null) => void;
  setSelectedNode: (nodeId: string, nodeType: Exclude<SelectedNodeType, null>) => void;
  clearSelectedNode: () => void;
  setSelectionPinned: (pinned: boolean) => void;
  setLastSeenEvent: (runId: string, eventId?: string) => void;
}

export const useUiStore = create<UiState>((set) => ({
  selectedRunId: null,
  selectedNodeId: null,
  selectedNodeType: null,
  isSelectionPinned: false,
  lastSeenEventByRun: {},
  setSelectedRunId: (selectedRunId) => set({ selectedRunId }),
  setSelectedNode: (selectedNodeId, selectedNodeType) => set({ selectedNodeId, selectedNodeType }),
  clearSelectedNode: () => set({ selectedNodeId: null, selectedNodeType: null }),
  setSelectionPinned: (isSelectionPinned) => set({ isSelectionPinned }),
  setLastSeenEvent: (runId, eventId) =>
    set((s) => ({
      lastSeenEventByRun: {
        ...s.lastSeenEventByRun,
        [runId]: eventId
      }
    }))
}));
