import { create } from "zustand";

// Transient (non-persisted) hover state shared across columns and the
// context growth widget. When a span row is hovered in the TRACES
// column, we publish the chat-ancestor span_pk so the widget can
// highlight the matching column.
interface HoverState {
  hoveredChatPk: number | null;
  setHoveredChatPk: (pk: number | null) => void;
}

export const useHoverState = create<HoverState>((set) => ({
  hoveredChatPk: null,
  setHoveredChatPk: (pk) => set({ hoveredChatPk: pk }),
}));
