import { toast } from 'sonner'
import { DotsThreeIcon } from '@phosphor-icons/react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

export function CopyMenu({
  summary,
  json,
  label,
}: Readonly<{ summary: string; json: string; label: string }>) {
  const onCopySummary = async () => {
    const ok = await navigator.clipboard?.writeText(summary).then(
      () => true,
      () => false,
    )
    toast[ok ? 'success' : 'error'](ok ? `${label} summary copied` : `Copy failed`)
  }

  const onCopyJson = async () => {
    const ok = await navigator.clipboard?.writeText(json).then(
      () => true,
      () => false,
    )
    toast[ok ? 'success' : 'error'](ok ? `${label} JSON copied` : `Copy failed`)
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={`Copy ${label}`}
        className="text-foreground hover:bg-muted focus-visible:border-ring focus-visible:ring-ring/50 inline-flex size-7 items-center justify-center rounded-none border border-transparent focus-visible:ring-1"
      >
        <DotsThreeIcon className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuItem onClick={onCopySummary}>Copy summary</DropdownMenuItem>
        <DropdownMenuItem onClick={onCopyJson}>Copy JSON</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
