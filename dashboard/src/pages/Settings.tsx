import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'

export default function Settings() {
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Settings</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">Settings coming soon.</p>
      </CardContent>
    </Card>
  )
}
