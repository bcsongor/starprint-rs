import { PrintedText } from "@/components/printed-text";
import type { Align, QrLayout } from "@/lib/api";
import { PX_PER_MM } from "@/lib/paper";

interface Props {
  layout: QrLayout | null;
  /** Data too long to encode; the symbol cannot be drawn at all. */
  error: string | null;
}

const ROW: Record<Align, string> = {
  left: "justify-start",
  center: "justify-center",
  right: "justify-end",
};

/**
 * The dark modules as horizontal runs, so a symbol is tens of rects
 * rather than the better part of a thousand.
 */
function runs(matrix: boolean[], modules: number) {
  const out: { x: number; y: number; width: number }[] = [];
  for (let y = 0; y < modules; y++) {
    let start = -1;
    for (let x = 0; x <= modules; x++) {
      const dark = x < modules && matrix[y * modules + x];
      if (dark && start < 0) start = x;
      if (!dark && start >= 0) {
        out.push({ x: start, y, width: x - start });
        start = -1;
      }
    }
  }
  return out;
}

/**
 * Draws the code as it will print: the caption, then the modules the
 * printer is given, at the millimetres they come out. The quiet zone is
 * part of the block, so the symbol keeps its margin against a caption.
 */
export function QrPreview({ layout, error }: Props) {
  if (error) {
    return <p className="py-4 text-center text-sm text-destructive">{error}</p>;
  }
  if (!layout) return null;

  return (
    <>
      {layout.caption.length > 0 && (
        <PrintedText columns={layout.columns}>
          {layout.caption.map((line, index) => (
            <div key={index} style={{ textAlign: layout.align }}>
              {line}
            </div>
          ))}
        </PrintedText>
      )}
      <div className={`flex ${ROW[layout.align]}`}>
        {/* Not square on the SP700, and by design: a module there is a
            whole number of dots on a head whose own dots are not. */}
        <svg
          width={layout.widthMm * PX_PER_MM}
          height={layout.heightMm * PX_PER_MM}
          viewBox={`0 0 ${layout.modules} ${layout.modules}`}
          preserveAspectRatio="none"
          shapeRendering="crispEdges"
          fill="currentColor"
        >
          {runs(layout.matrix, layout.modules).map(({ x, y, width }) => (
            <rect key={`${x}-${y}`} x={x} y={y} width={width} height={1} />
          ))}
        </svg>
      </div>
    </>
  );
}
