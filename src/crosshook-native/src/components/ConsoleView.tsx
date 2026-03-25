import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type UIEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";

type ConsoleLine = {
  id: number;
  timestamp: string;
  text: string;
};

function formatTimestamp(date: Date) {
  return date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function normalizeLogMessage(payload: unknown) {
  if (typeof payload === "string") {
    return payload;
  }

  if (payload === null || typeof payload !== "object") {
    return "";
  }

  const record = payload as Record<string, unknown>;

  if (
    "line" in record &&
    typeof record.line === "string"
  ) {
    return record.line;
  }

  if (
    "message" in record &&
    typeof record.message === "string"
  ) {
    return record.message;
  }

  if (
    "text" in record &&
    typeof record.text === "string"
  ) {
    return record.text;
  }

  return "";
}

export function ConsoleView() {
  const [collapsed, setCollapsed] = useState(false);
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const nextId = useRef(1);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const shouldFollowRef = useRef(true);

  function scrollToBottom() {
    const body = bodyRef.current;
    if (!body) {
      return;
    }

    body.scrollTop = body.scrollHeight;
  }

  function updateFollowState(element: HTMLDivElement) {
    const distanceFromBottom =
      element.scrollHeight - (element.scrollTop + element.clientHeight);
    shouldFollowRef.current = distanceFromBottom <= 24;
  }

  function handleBodyScroll(event: UIEvent<HTMLDivElement>) {
    updateFollowState(event.currentTarget);
  }

  useEffect(() => {
    let active = true;

    const unlistenPromise = listen<unknown>("launch-log", (event) => {
      const text = normalizeLogMessage(event.payload).trimEnd();
      if (!text) {
        return;
      }

      const entry = {
        id: nextId.current++,
        timestamp: formatTimestamp(new Date()),
        text,
      };

      if (active) {
        setLines((current) => [...current, entry]);
      }
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useLayoutEffect(() => {
    if (!collapsed && shouldFollowRef.current) {
      scrollToBottom();
    }
  }, [collapsed, lines.length]);

  return (
    <section
      aria-label="Console log"
      style={{
        border: "1px solid rgba(96, 165, 250, 0.22)",
        borderRadius: "18px",
        background:
          "linear-gradient(180deg, rgba(10, 14, 24, 0.98), rgba(6, 10, 18, 0.98))",
        boxShadow: "0 24px 60px rgba(0, 0, 0, 0.35)",
        overflow: "hidden",
      }}
    >
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "16px",
          padding: "16px 18px",
          borderBottom: "1px solid rgba(148, 163, 184, 0.12)",
        }}
      >
        <div>
          <div
            style={{
              color: "#dbeafe",
              fontSize: "0.72rem",
              letterSpacing: "0.18em",
              textTransform: "uppercase",
            }}
          >
            Runtime Console
          </div>
          <div
            style={{
              color: "#f8fafc",
              fontSize: "1rem",
              fontWeight: 600,
              marginTop: "4px",
            }}
          >
            Helper log stream
          </div>
        </div>

        <div style={{ display: "flex", gap: "10px" }}>
          <button
            type="button"
            onClick={() => setLines([])}
            style={buttonStyle}
          >
            Clear
          </button>
          <button
            type="button"
            onClick={() => setCollapsed((value) => !value)}
            style={buttonStyle}
          >
            {collapsed ? "Expand" : "Collapse"}
          </button>
        </div>
      </header>

      {!collapsed ? (
        <div
		ref={bodyRef} onScroll={handleBodyScroll}
          style={{
            minHeight: "280px",
            maxHeight: "52vh",
            overflowY: "auto",
            padding: "18px",
            background:
              "radial-gradient(circle at top right, rgba(59, 130, 246, 0.08), transparent 28%), rgba(2, 6, 23, 0.92)",
            color: "#e2e8f0",
            fontFamily:
              '"SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace',
            fontSize: "0.9rem",
            lineHeight: 1.55,
          }}
        >
          {lines.length === 0 ? (
            <div
              style={{
                display: "grid",
                gap: "10px",
                placeItems: "center",
                minHeight: "220px",
                textAlign: "center",
                color: "#94a3b8",
                border: "1px dashed rgba(148, 163, 184, 0.24)",
                borderRadius: "14px",
                background: "rgba(15, 23, 42, 0.45)",
                padding: "24px",
              }}
            >
              <div style={{ fontSize: "1rem", color: "#e2e8f0" }}>
                Waiting for log output
              </div>
              <div style={{ maxWidth: "34rem" }}>
                Launch a game or trainer to stream helper output here. New
                lines appear automatically as CrossHook emits `launch-log`
                events.
              </div>
            </div>
          ) : (
            <div style={{ display: "grid", gap: "10px" }}>
              {lines.map((line) => (
                <div
                  key={line.id}
                  style={{
                    display: "grid",
                    gridTemplateColumns: "88px 1fr",
                    gap: "12px",
                    alignItems: "start",
                    padding: "10px 12px",
                    borderRadius: "12px",
                    background: "rgba(15, 23, 42, 0.7)",
                    border: "1px solid rgba(148, 163, 184, 0.12)",
                  }}
                >
                  <span style={{ color: "#60a5fa" }}>[{line.timestamp}]</span>
                  <pre
                    style={{
                      margin: 0,
                      whiteSpace: "pre-wrap",
                      wordBreak: "break-word",
                    }}
                  >
                    {line.text}
                  </pre>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : null}
    </section>
  );
}

const buttonStyle: CSSProperties = {
  appearance: "none",
  border: "1px solid rgba(96, 165, 250, 0.3)",
  borderRadius: "999px",
  background: "rgba(30, 41, 59, 0.85)",
  color: "#f8fafc",
  cursor: "pointer",
  fontSize: "0.88rem",
  fontWeight: 600,
  padding: "9px 14px",
};

export default ConsoleView;
