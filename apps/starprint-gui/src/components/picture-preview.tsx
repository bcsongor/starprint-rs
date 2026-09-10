import { PreviewNote, PreviewPane } from "@/components/preview-pane";
import type { Profile } from "@/lib/api";
import { roll } from "@/lib/paper";

interface Props {
  /** A `data:` PNG, or null while there is no picture. */
  image: string | null;
  error: string | null;
  printer: Profile;
}

/**
 * The dithered picture as the paper will show it. The pipeline's preview
 * is already square-pixelled at the head's single-density width, so it
 * fills the print region at its own aspect ratio.
 */
export function PicturePreview({ image, error, printer }: Props) {
  return (
    <PreviewPane printer={printer} hint={`${roll(printer).dots} dots`}>
      {error ? (
        <PreviewNote error>{error}</PreviewNote>
      ) : image ? (
        <img src={image} alt="Dithered preview" className="block w-full" />
      ) : (
        <PreviewNote>Choose a picture to see how it will print.</PreviewNote>
      )}
    </PreviewPane>
  );
}
