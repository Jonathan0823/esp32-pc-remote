import type { ComponentType } from 'react'
import { SidebarMenuItem, SidebarMenuButton } from '@/components/ui/sidebar'

export function NavItem({
  icon: Icon,
  label,
  active,
  onClick,
}: Readonly<{
  icon: ComponentType<{ className?: string }>
  label: string
  active?: boolean
  onClick?: () => void
}>) {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} tooltip={label} onClick={onClick}>
        <Icon className="size-4" />
        <span>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}
