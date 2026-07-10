import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'

export default function About() {
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>About</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">
          PC Remote Control Panel &middot; ESP32-based Wake-on-LAN controller.
        </p>
      </CardContent>
    </Card>
  )
}
