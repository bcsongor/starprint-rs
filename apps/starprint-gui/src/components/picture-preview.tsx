import { cn } from "@/lib/utils";

interface Props {
  /** Blob URL of the dithered PNG, or null before a picture is chosen. */
  url: string | null;
  /** Why there is no preview, when there is none. */
  error: string | null;
}

/**
 * The dithered picture as the paper will show it. The pipeline's preview
 * is already square-pixelled at the head's single-density width, so it
 * fills the print region at its own aspect ratio.
 */
export function PicturePreview({ url, error }: Props) {
  if (!url) {
    return (
      // No size of its own: the sheet sets it, so the message reads at
      // the printer's normal text size like the card's placeholder does.
      <p
        className={cn(
          "py-[1em] text-center",
          error ? "text-red-700" : "text-black/30",
        )}
      >
        {error ?? "Choose a picture to see how it will print."}
      </p>
    );
  }
  return <img src={url} alt="Dithered preview" className="block w-full" />;
}
