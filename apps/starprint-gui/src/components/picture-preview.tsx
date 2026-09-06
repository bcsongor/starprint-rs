import { PreviewPane } from "@/components/preview-pane";
import { usePreview } from "@/hooks/use-preview";
import {
  picturePreview,
  type Paper,
  type Picture,
  type PrinterKind,
} from "@/lib/api";
import { roll } from "@/lib/paper";
import { cn } from "@/lib/utils";

interface Props {
  picture: Picture;
  kind: PrinterKind;
  paper: Paper;
}

/**
 * The dithered picture as the paper will show it. The pipeline's preview
 * is already square-pixelled at the head's single-density width, so it
 * fills the print region at its own aspect ratio.
 */
export function PicturePreview({ picture, kind, paper }: Props) {
  const { url, error } = usePreview(
    async () => {
      if (picture.path === "") return null;
      return { value: null, png: await picturePreview(picture, kind, paper) };
    },
    [picture, kind, paper],
  );

  return (
    <PreviewPane
      printer={{ kind, paper }}
      hint={`${roll(kind, paper).dots} dots`}
    >
      {url ? (
        <img src={url} alt="Dithered preview" className="block w-full" />
      ) : (
        <p
          className={cn(
            "py-[1em] text-center text-xl",
            error ? "text-red-700" : "text-black/30",
          )}
        >
          {error ?? "Choose a picture to see how it will print."}
        </p>
      )}
    </PreviewPane>
  );
}
