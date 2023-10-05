import { HStack, Heading } from '@chakra-ui/react'
import Image from 'next/image'

export function Logo({ fontSize = '3xl', size = 50 }) {
  return (
    <HStack>
      <Image src='/img/logo.svg' alt='Polylang' width={size} height={size} />
      <Heading as='h1' fontSize={fontSize} fontWeight={600}>
        Polylang
      </Heading>
    </HStack>
  )
}