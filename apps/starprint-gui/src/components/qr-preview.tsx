import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import { usePreview } from "@/hooks/use-preview";
import {
  QUIET_MODULES,
  qrLayout,
  qrPreview,
  type Align,
  type Paper,
  type PrinterKind,
  type Qr,
} from "@/lib/api";
import { PX_PER_MM } from "@/lib/paper";

interface Props {
  code: Qr;
  kind: PrinterKind;
  paper: Paper;
}

const ROW: Record<Align, string> = {
  left: "justify-start",
  center: "justify-center",
  right: "justify-end",
};

/**
 * Draws the code as it will print: the caption, then the symbol the
 * printer is given, at the millimetres it comes out. The quiet zone is
 * part of the block, so the symbol keeps its margin against a caption.
 */
export function QrPreview({ code, kind, paper }: Props) {
  const {
    value: layout,
    url,
    error,
  } = usePreview(
    async () => {
      // Empty data encodes a valid symbol, but there is nothing to print yet.
      if (!code.data.trim()) return null;
      const [value, png] = await Promise.all([
        qrLayout(code, kind, paper),
        qrPreview(code, kind, paper),
      ]);
      return { value, png };
    },
    [code, kind, paper],
  );

  // Size includes the quiet zone; the module count describes the symbol.
  const hint =
    layout &&
    `${layout.modules - 2 * QUIET_MODULES} modules, ${Math.round(layout.widthMm)} mm`;

  return (
    <PreviewPane printer={{ kind, paper }} hint={hint}>
      {error ? (
        <p className="py-4 text-center text-sm text-destructive">{error}</p>
      ) : layout && url ? (
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
            {/* Sized in millimetres rather than at the PNG's own pixels:
            on the SP700 a module is a whole number of dots on a head
            whose dots are far from square. */}
            <img
              src={url}
              alt="QR code preview"
              className="block"
              width={layout.widthMm * PX_PER_MM}
              height={layout.heightMm * PX_PER_MM}
            />
          </div>
        </>
      ) : null}
    </PreviewPane>
  );
}
