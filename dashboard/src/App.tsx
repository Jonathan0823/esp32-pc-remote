import { BrowserRouter, Routes, Route } from 'react-router-dom'
import { ThemeProvider } from '@/components/ui/theme-provider'
import { TooltipProvider } from '@/components/ui/tooltip'
import { Toaster } from '@/components/ui/sonner'
import Layout from '@/components/Layout'
import Dashboard from '@/pages/Dashboard'
import Activity from '@/pages/Activity'
import Settings from '@/pages/Settings'
import About from '@/pages/About'
import './App.css'

function AppWrapper() {
  return (
    <ThemeProvider defaultTheme="dark" storageKey="pc-remote-theme">
      <BrowserRouter>
        <TooltipProvider>
          <Routes>
            <Route element={<Layout />}>
              <Route index element={<Dashboard />} />
              <Route path="activity" element={<Activity />} />
              <Route path="settings" element={<Settings />} />
              <Route path="about" element={<About />} />
            </Route>
          </Routes>
          <Toaster position="bottom-right" />
        </TooltipProvider>
      </BrowserRouter>
    </ThemeProvider>
  )
}

export default AppWrapper
