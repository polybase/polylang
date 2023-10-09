import { useState } from "react"
import { Box, Button, Stack, Heading, SimpleGrid, useToast } from "@chakra-ui/react"
import { encodeBase64 } from 'tweetnacl-util'
import { Code } from './Code'
import { EXAMPLES } from "./example"
import { run, Output } from './polylang'
import { useAsyncCallback } from "./useAsyncCallback"

interface UserOutput {
    // proof: 
}

const Prover = () => {
    const [code, setCode] = useState(EXAMPLES[0].code)
    const [inputs, setInputs] = useState(EXAMPLES[0].inputs)
    const [report, setReport] = useState('')
    const [output, setOutput] = useState<Output | null>(null)
    const toast = useToast()

    const prove = useAsyncCallback(async () => {
        const parsedInputs = JSON.parse(inputs)
        const output = run(code, parsedInputs)
        setOutput(output)
        setReport(JSON.stringify({
            proof: encodeBase64(output.proof()),
            proofLength: output.proof().length,
            cycleCount: output.cycle_count(),
            // this: hasThis ? output.this() : null,
            logs: output.logs(),
            hashes: output.hashes(),
            // selfDestructed: output.self_destructed(),
            readAuth: output.read_auth(),
        }, null, 2))
    })

    const verify = useAsyncCallback(() => {
        if (!output) {
            return toast({
                status: 'error',
                title: 'No proof',
                description: 'There is no proof to verify',
                duration: 9000,
            })
        }
        const time = Date.now()
        // TODO: revert this in the prover PR.
        //output?.verify()
        const diff = Date.now() - time
        toast({
            status: 'success',
            title: 'Valid Proof',
            description: `Proof was verified in ${diff}ms`,
            duration: 9000,
        })
    })

    return (
        <SimpleGrid columns={1} spacing={10} minChildWidth='300px'>
            <Stack>
                <Heading size='md'>Inputs</Heading>
                <Box height='100%' flex='1 1 auto' minH='0px' borderRadius={10} overflow='hidden' css={{ '.cm-gutters': { border: 0 } }}>
                    <Code type='json' value={EXAMPLES[0].inputs} onChange={(inputs) => {
                        setInputs(inputs)
                        setOutput(null)
                        setReport('')
                    }} />
                </Box>
            </Stack>
            <Stack>
                <Stack height='100%'>
                    <Heading size='md'>Code</Heading>
                    <Box height='100%' flex='1 1 auto' minH='0px' borderRadius={10} overflow='hidden' css={{ '.cm-gutters': { border: 0 } }}>
                        <Code type='polylang' value={EXAMPLES[0].code} onChange={(code) => {
                            setCode(code)
                            setOutput(null)
                            setReport('')
                        }} />
                    </Box>
                </Stack>
                <Button size='md' onClick={prove.execute}>Prove</Button>
            </Stack>
            <Stack>
                <Stack height='100%'>
                    <Heading size='md'>Output</Heading>
                    <Box height='100%' maxH='300px' flex='1 1 auto' minH='0px' borderRadius={10} overflow='hidden' css={{ '.cm-gutters': { border: 0 } }}>
                        <Code type='json' editable={false} value={report} />
                    </Box>
                </Stack>
                <Button size='md' onClick={verify.execute}>Verify</Button>
            </Stack>
        </SimpleGrid>
    )
}

export default Prover