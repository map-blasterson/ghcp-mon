import { create } from "zustand";

// Transient (non-persisted) hover state shared across columns and the
// context growth widget. When a span row is hovered in the TRACES
// column, we publish the chat-ancestor span_pk so the widget can
// highlight the matching column.
//
// clickedChat is a fire-and-forget signal: set when a bar in the
// context growth chart is clicked, consumed and cleared by SpansScenario
// to drive selection.
interface ClickedChat {
  traceId: string;
  spanId: string;
}

interface HoverState {
  hoveredChatPk: number | null;
  setHoveredChatPk: (pk: number | null) => void;
  clickedChat: ClickedChat | null;
  setClickedChat: (v: ClickedChat | null) => void;
}

export const useHoverState = create<HoverState>((set) => ({
  hoveredChatPk: null,
  setHoveredChatPk: (pk) => set({ hoveredChatPk: pk }),
  clickedChat: null,
  setClickedChat: (v) => set({ clickedChat: v }),
}));
