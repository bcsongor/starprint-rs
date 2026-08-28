import * as React from "react"
import { useRender } from "@base-ui/react/use-render"

import { cn } from "@/lib/utils"

function Label({
  className,
  render,
  ...props
}: React.ComponentProps<"label"> & useRender.ComponentProps<"label">) {
  return useRender({
    defaultTagName: "label",
    render,
    props: {
      "data-slot": "label",
      className: cn(
        // Utilitarian: small caps-style tracker in muted grey, so labels
        // sit under the 14px control text rather than compete with it.
        "flex items-center gap-2 text-[11px] leading-none font-medium tracking-wider uppercase text-muted-foreground select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
        className
      ),
      ...props,
    },
  })
}

export { Label }
