import { Card, Text, Heading, SimpleGrid, Stack, useColorModeValue } from "@chakra-ui/react"

const WhyBlock = ({ title, children }) => {
    return (
        <Card background='bw.50'>
            <Stack p={3}>
                <Heading as='h3' size='md'>{title}</Heading>
                <Text lineHeight={1.8} fontSize={'md'}>{children}</Text>
            </Stack>
        </Card>
    )
}

const Why = () => {
    return (
        <Stack spacing='10'>
            <Heading as='h2'>Why?</Heading>
            <SimpleGrid columns={1} spacing={10} minChildWidth='300px'>
                <WhyBlock title='Verifiable computation'>
                    Run computations and generate a proof that the computation was run as designed.
                </WhyBlock>
                <WhyBlock title='Zero knowledge'>
                    Hide the inputs/outputs to the program while ensuring , to enable end user privacy.
                </WhyBlock>
                <WhyBlock title='Contracts'>
                    Define state (similar to TypeScript class or Solidity contract) to allow provable state transitions.
                </WhyBlock>
                <WhyBlock title='Automatic hashing'>
                    Inputs to functions (and contract state) are hashed, allowing commitments to inputs/outputs to be made.
                </WhyBlock>
                <WhyBlock title='Works in the browser'>
                    Compiles to WASM so computation and proof generation, as well as verification, can be done in the browser.
                </WhyBlock>
                <WhyBlock title='JavaScript/TypeScript'>
                    The language you know and love already, ported to run in zero knowledge.
                </WhyBlock>
            </SimpleGrid >
        </Stack >
    )
}

export default Why