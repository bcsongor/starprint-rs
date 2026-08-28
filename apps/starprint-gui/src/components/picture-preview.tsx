interface Props {
  /** Blob URL of the dithered PNG, or null before a picture is chosen. */
  url: string | null;
  /** Why there is no preview, when there is none. */
  error: string | null;
}

/**
 * The dithered picture as the paper will show it: single-density width
 * with square pixels, scaled to fit the pane without scrolling.
 */
export function PicturePreview({ url, error }: Props) {
  if (!url) {
    return (
      <p className="text-sm text-muted-foreground">
        {error ?? "Choose a picture to see how it will print."}
      </p>
    );
  }
  return (
    <div className="flex min-h-0 flex-1 items-start">
      <img
        src={url}
        alt="Dithered preview"
        className="max-h-full max-w-full bg-white object-contain object-left-top shadow-sm"
      />
    </div>
  );
}
