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

/** Short enough to keep the path string of a dense symbol sane. */
const round = (value: number) => +value.toFixed(3);

/**
 * The dark modules as a single path, one rounded rectangle each, so a
 * symbol is one element rather than the better part of a thousand.
 *
 * A corner is curved only where both of the modules it faces are light,
 * which is the rule the printer draws its dots with: a run of dark
 * modules stays joined and it is the outside of the run that curves.
 */
function path(matrix: boolean[], modules: number, rx: number, ry: number) {
  const dark = (x: number, y: number) =>
    x >= 0 && x < modules && y >= 0 && y < modules && matrix[y * modules + x];
  // Elliptical, the radii differing as a module's dots do.
  const arc = (sx: number, sy: number) =>
    `a${round(rx)} ${round(ry)} 0 0 1 ${round(sx * rx)} ${round(sy * ry)}`;

  let d = "";
  for (let y = 0; y < modules; y++) {
    for (let x = 0; x < modules; x++) {
      if (!dark(x, y)) continue;

      const curved = (hx: number, hy: number) =>
        rx > 0 && !dark(x + hx, y) && !dark(x, y + hy);
      const [tl, tr, br, bl] = [
        curved(-1, -1),
        curved(1, -1),
        curved(1, 1),
        curved(-1, 1),
      ];
      // Clockwise from the top-left, cutting each curved corner.
      d +=
        `M${round(x + (tl ? rx : 0))} ${y}` +
        `H${round(x + 1 - (tr ? rx : 0))}${tr ? arc(1, 1) : ""}` +
        `V${round(y + 1 - (br ? ry : 0))}${br ? arc(-1, 1) : ""}` +
        `H${round(x + (bl ? rx : 0))}${bl ? arc(-1, -1) : ""}` +
        `V${round(y + (tl ? ry : 0))}${tl ? arc(1, -1) : ""}Z`;
    }
  }
  return d;
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
          shapeRendering={layout.radiusX > 0 ? "geometricPrecision" : "crispEdges"}
          fill="currentColor"
        >
          <path
            d={path(
              layout.matrix,
              layout.modules,
              layout.radiusX,
              layout.radiusY,
            )}
          />
        </svg>
      </div>
    </>
  );
}
