import { PrintedText } from "@/components/printed-text";
import { PreviewNote, PreviewPane } from "@/components/preview-pane";
import {
  QUIET_MODULES,
  type Align,
  type Profile,
  type QrLayout,
} from "@/lib/api";
import { PX_PER_MM } from "@/lib/paper";

interface Props {
  preview: (QrLayout & { image: string }) | null;
  error: string | null;
  printer: Profile;
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
export function QrPreview({ preview, error, printer }: Props) {
  // Size includes the quiet zone; the module count describes the symbol.
  const hint =
    preview &&
    `${preview.modules - 2 * QUIET_MODULES} modules, ${Math.round(preview.widthMm)} mm`;

  return (
    <PreviewPane printer={printer} hint={hint}>
      {error ? (
        <PreviewNote error>{error}</PreviewNote>
      ) : preview ? (
        <>
          {preview.caption.length > 0 && (
            <PrintedText columns={preview.columns}>
              {preview.caption.map((line, index) => (
                <div key={index} style={{ textAlign: preview.align }}>
                  {line}
                </div>
              ))}
            </PrintedText>
          )}
          <div className={`flex ${ROW[preview.align]}`}>
            {/* Sized in millimetres rather than at the PNG's own pixels:
            on the SP700 a module is a whole number of dots on a head
            whose dots are far from square. */}
            <img
              src={preview.image}
              alt="QR code preview"
              className="block"
              width={preview.widthMm * PX_PER_MM}
              height={preview.heightMm * PX_PER_MM}
            />
          </div>
        </>
      ) : null}
    </PreviewPane>
  );
}
