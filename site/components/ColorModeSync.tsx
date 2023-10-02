import { useEffect } from 'react'
import { useColorMode } from '@chakra-ui/react'
import { useTheme } from 'nextra-theme-docs'

export function ColorModeSync() {
  const { colorMode, setColorMode } = useColorMode()
  const theme = useTheme()

  useEffect(() => {
    if (theme.resolvedTheme && colorMode !== theme.resolvedTheme) {
      setColorMode(theme.resolvedTheme)
    }
  }, [colorMode, theme.resolvedTheme, setColorMode])

  return null
}