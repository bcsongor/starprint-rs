import type { ComponentProps } from "react"
import {
  DayButton,
  DayPicker,
  getDefaultClassNames,
  type PropsBase,
  type PropsSingle,
} from "react-day-picker"
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import { buttonVariants } from "@/components/ui/button-variants"

function Calendar({
  className,
  ...props
}: Pick<PropsBase, "className" | "defaultMonth" | "weekStartsOn"> &
  Pick<PropsSingle, "selected" | "onSelect">) {
  const defaultClassNames = getDefaultClassNames()

  return (
    <DayPicker
      mode="single"
      showOutsideDays
      className={cn(
        "p-2 [--cell-radius:var(--radius-md)] [--cell-size:--spacing(7)]",
        className
      )}
      classNames={{
        root: cn("w-fit", defaultClassNames.root),
        months: cn("relative", defaultClassNames.months),
        month: cn("flex w-full flex-col gap-4", defaultClassNames.month),
        nav: cn(
          "absolute inset-x-0 top-0 flex w-full items-center justify-between gap-1",
          defaultClassNames.nav
        ),
        button_previous: cn(
          buttonVariants({ variant: "ghost" }),
          "size-(--cell-size) p-0 select-none aria-disabled:opacity-50",
          defaultClassNames.button_previous
        ),
        button_next: cn(
          buttonVariants({ variant: "ghost" }),
          "size-(--cell-size) p-0 select-none aria-disabled:opacity-50",
          defaultClassNames.button_next
        ),
        month_caption: cn(
          "flex h-(--cell-size) w-full items-center justify-center px-(--cell-size)",
          defaultClassNames.month_caption
        ),
        caption_label: cn(
          "text-sm font-medium select-none",
          defaultClassNames.caption_label
        ),
        month_grid: cn("w-full border-collapse", defaultClassNames.month_grid),
        weekdays: cn("flex", defaultClassNames.weekdays),
        weekday: cn(
          "flex-1 rounded-(--cell-radius) text-[0.8rem] font-normal text-muted-foreground select-none",
          defaultClassNames.weekday
        ),
        week: cn("mt-2 flex w-full", defaultClassNames.week),
        day: cn(
          "group/day relative aspect-square h-full w-full rounded-(--cell-radius) p-0 text-center select-none",
          defaultClassNames.day
        ),
        today: cn(
          "rounded-(--cell-radius) bg-muted text-foreground",
          defaultClassNames.today
        ),
        outside: cn("text-muted-foreground", defaultClassNames.outside),
        disabled: cn("text-muted-foreground opacity-50", defaultClassNames.disabled),
        hidden: cn("invisible", defaultClassNames.hidden),
      }}
      components={{
        Chevron: ({ className, orientation, ...props }) => {
          const Icon = orientation === "left" ? ChevronLeftIcon : ChevronRightIcon
          return <Icon className={cn("size-4", className)} {...props} />
        },
        DayButton: CalendarDayButton,
      }}
      {...props}
    />
  )
}

function CalendarDayButton({
  className,
  modifiers,
  ...props
}: ComponentProps<typeof DayButton>) {
  return (
    <DayButton
      modifiers={modifiers}
      data-selected={modifiers.selected}
      className={cn(
        buttonVariants({ variant: "ghost", size: "icon" }),
        "relative z-10 aspect-square size-auto w-full min-w-(--cell-size) border-0 leading-none font-normal group-data-[focused=true]/day:ring-[3px] group-data-[focused=true]/day:ring-ring/50 data-[selected=true]:bg-primary data-[selected=true]:text-primary-foreground",
        className
      )}
      {...props}
    />
  )
}

export { Calendar }
