import { PreviewPane } from "@/components/preview-pane";
import { useAsync } from "@/hooks/use-async";
import { testPageSections, type PrinterKind, type TestPage } from "@/lib/api";

interface Props {
  page: TestPage;
  kind: PrinterKind;
}

/**
 * The test page is mostly raster, so the preview is its outline: the
 * numbered sections as they will print, each with what a fault looks
 * like on its own line.
 */
export function TestPagePreview({ page, kind }: Props) {
  const sections = useAsync(() => testPageSections(page, kind), [page, kind]);
  return (
    <PreviewPane>
      <ol className="grid gap-1.5 text-sm leading-snug">
        {sections?.map((section, index) => (
          <li
            key={section.title}
            className="grid grid-cols-[1.25rem_1fr] gap-1"
          >
            <span className="font-mono text-muted-foreground">{index + 1}</span>
            <span className="grid">
              <span>{section.title}</span>
              <span className="text-xs text-muted-foreground">
                {section.check}
              </span>
            </span>
          </li>
        ))}
      </ol>
    </PreviewPane>
  );
}
