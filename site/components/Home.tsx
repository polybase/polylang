'use client'

import { Button, Link, HStack, Box, Stack, Heading } from "@chakra-ui/react"
import { useTheme } from 'nextra-theme-docs'
import Prover from './Prover'
import Why from './Why'
import Navbar from './Navbar'
import { ColorModeSync } from "./ColorModeSync"

const Home = ({ children }) => {
    return (
        <Box maxW='container.lg' margin='0 auto'>
            <ColorModeSync />
            <Stack spacing='6em'>
                <Box py={3}>
                    <Navbar />
                </Box>
                <Stack spacing='10em'>
                    <Stack spacing={6}>
                        <Heading as='h1' fontSize={['3em', '4em']}>
                            TypeScript for<br /> Zero Knowledge
                        </Heading>
                        <Heading as='h2' fontSize='xl'>Provable computation and zero knowledge language.</Heading>
                        <Stack spacing={10}>
                            <HStack>
                                <Button size='lg' as={Link} href='/docs'>Get Started</Button>
                                <Button size='lg' as={Link} href='/playground'>Playground</Button>
                            </HStack>
                        </Stack>
                    </Stack>
                    <Stack spacing={4}>
                        <Heading as='h2' fontSize='2em'>Try it out</Heading>
                        <Prover />
                    </Stack>
                    <Why />
                    <Box>
                        <Heading as='h2'>FAQ</Heading>
                        {children}
                    </Box>
                </Stack>
            </Stack>
        </Box>
    )
}

export default Home