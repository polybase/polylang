import * as React from 'react'
import {
  useColorMode,
  useColorModeValue,
  IconButton,
  IconButtonProps,
} from '@chakra-ui/react'
import { useTheme } from 'nextra-theme-docs'
import { FaMoon, FaSun } from 'react-icons/fa'

type ColorModeSwitcherProps = Omit<IconButtonProps, 'aria-label'>

export const ColorModeSwitcher: React.FC<ColorModeSwitcherProps> = (props) => {
  const theme = useTheme()
  const { setColorMode } = useColorMode()
  const text = useColorModeValue('dark', 'light')
  const SwitchIcon = useColorModeValue(FaMoon, FaSun)

  return (
    <IconButton
      size='md'
      fontSize='lg'
      variant='ghost'
      color='current'
      marginLeft='2'
      _hover={{ color: 'brand.500' }}
      onClick={() => {
        theme.setTheme(theme.resolvedTheme === 'dark' ? 'light' : 'dark')
        setColorMode(theme.resolvedTheme === 'dark' ? 'light' : 'dark')
      }}
      icon={<SwitchIcon />}
      aria-label={`Switch to ${text} mode`}
      {...props}
    />
  )
}
