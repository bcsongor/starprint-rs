import { PrintedText } from "@/components/printed-text";
import type { Align, QrLayout } from "@/lib/api";
import { PX_PER_MM } from "@/lib/paper";

interface Props {
  layout: QrLayout | null;
  /** Blob URL of the symbol's PNG, drawn from the dots that print. */
  url: string | null;
  /** Data too long to encode; the symbol cannot be drawn at all. */
  error: string | null;
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
export function QrPreview({ layout, url, error }: Props) {
  if (error) {
    return <p className="py-4 text-center text-sm text-destructive">{error}</p>;
  }
  if (!layout || !url) return null;

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
  );
}
