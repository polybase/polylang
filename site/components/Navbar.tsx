'use client'

import Image from 'next/image'
import { Flex, HStack, Heading, Link, Spacer } from '@chakra-ui/react'
import { PoweredBy } from './PoweredBy'
import { Logo } from './Logo'
import { ColorModeSwitcher } from './ColorModeSwitcher'

const Navbar = () => {
  return (
    <Flex as='nav' py={2}>
      <Logo />
      <Spacer />
      <HStack spacing={6}>
        <ColorModeSwitcher />
        <Link href='/playground'>Playground</Link>
        <Link href='/docs'>Docs</Link>
        <Link href='https://github.com/polybase/polylang'>Github</Link>
        <PoweredBy />
      </HStack>
    </Flex >
  )
}

export default Navbar