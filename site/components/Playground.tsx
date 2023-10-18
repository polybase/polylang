import { Box, Stack, HStack, Flex, Heading, Spacer, Button, Select, useToast, Alert, AlertIcon, Text } from '@chakra-ui/react'
import { Code } from './Code'
import { Logo } from './Logo'
import { useState } from 'react'
import { EXAMPLES } from './example'
import Link from 'next/link'
import { useAsyncCallback } from './useAsyncCallback'
import { compile, run, verify as verifyProof, Output, ServerOutput } from './polylang'
import { encodeBase64 } from 'tweetnacl-util'

function Panel({ heading, children }) {
  return (
    <Flex height='100%' flexDirection='column' borderRadius={5} overflow='hidden'>
      <Box bg='bw.50' p={2}>
        <Heading size='xs'>{heading}</Heading>
      </Box>
      <Box height='100%'>
        {children}
      </Box>
    </Flex>
  )
}

// Stringifying `Map` is not directly supported
const processThisVal = (thisVal: any): any => {
  const replace_fn = (_, value: any) => {
    return value instanceof Map ? Array.from(value.entries()) : value
  }

  return JSON.parse(JSON.stringify(thisVal, replace_fn))
}

export function Playground() {
  const [code, setCode] = useState(EXAMPLES[0].code)
  const [inputs, setInputs] = useState(EXAMPLES[0].inputs)
  const [report, setReport] = useState('')
  const [browserOutput, setBrowserOutput] = useState<Output>(null)
  const [serverOutput, setServerOutput] = useState<ServerOutput>(null)
  const toast = useToast()

  const clearOutput = useAsyncCallback(() => {
    setBrowserOutput(null)
    setServerOutput(null)
    setReport('')
  })

  // Uses the WASM API
  const prove_browser = useAsyncCallback(async () => {
    clearOutput.execute()

    const parsedInputs = JSON.parse(inputs)
    const output = run(code, parsedInputs)
    setBrowserOutput(output)

    const hasThis = parsedInputs.contract_name === '' ? false : true
    setReport(JSON.stringify({
      proof: encodeBase64(output.proof()),
      proofLength: output.proof().length,
      cycleCount: output.cycle_count(),
      logs: output.logs(),
      this: hasThis ? processThisVal(output.this()) : null,
      result: output.result(),
      result_hash: output.result_hash(),
      hashes: output.hashes(),
      selfDestructed: output.self_destructed(),
      readAuth: output.read_auth(),
    }, null, 2))
  })

  // Uses the /prove endpoint
  const prove_server = useAsyncCallback(() => {
    clearOutput.execute()

    const proverUrl = 'http://127.0.0.1:8080/prove'
    const parsedInputs = JSON.parse(inputs)

    const { midenCode, abi } = compile(code, parsedInputs)

    const payload = {
      midenCode: midenCode,
      abi: abi,
      this: parsedInputs.init_params,
      args: parsedInputs.params,
    }

    fetch(proverUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(payload),
    }).then(resp => resp.json())
      .then(output => {
        setServerOutput(output)

        const hasThis = parsedInputs.contract_name === '' ? false : true
        setReport(JSON.stringify({
          proof: output.proof,
          proofLength: output.proofLength,
          cycleCount: output.cycleCount,
          logs: output.logs,
          this: hasThis ? processThisVal(output.new.this) : null,
          result: output.result?.value,
          result_hash: output.result?.hash,
          hashes: output.new.hashes,
          selfDestructed: output.new.selfDestructed,
          readAuth: output.readAuth,
        }, null, 2))
      })
      .catch(err => console.log(err))
  })

  // common verify function that dispatches to either the
  // browser verifier or the server verifier
  const verify = useAsyncCallback(() => {
    if (!browserOutput && !serverOutput) {
      return toast({
        status: 'error',
        title: 'No proof',
        description: 'There is no proof to verify',
        duration: 9000,
      })
    } else if (browserOutput) {
      verify_browser.execute()
    } else {
      verify_server.execute()
    }
  })

  const verify_browser = useAsyncCallback(() => {
    const proof = browserOutput.proof()
    const programInfo = browserOutput.program_info()
    const stackInputs = browserOutput.stack_inputs()
    const outputStack = browserOutput.output_stack()
    const overflowAddrs = browserOutput.overflow_addrs()

    const time = Date.now()
    verifyProof(proof, programInfo, stackInputs, outputStack, overflowAddrs)
    const diff = Date.now() - time

    toast({
      status: 'success',
      title: 'Valid Proof',
      description: `Proof was verified in ${diff} ms`,
      duration: 9000,
    })
  })

  const verify_server = useAsyncCallback(() => {
    const proof = serverOutput.proof
    const programInfo = serverOutput.programInfo
    const stackInputs = serverOutput.stack.input
    const outputStack = serverOutput.stack.output
    const overflowAddrs = serverOutput.stack.overflowAddrs

    const time = Date.now()
    const proofBytes = new Uint8Array(atob(proof).split('').map(c => c.charCodeAt(0)))
    verifyProof(proofBytes, programInfo, stackInputs, outputStack, overflowAddrs)
    const diff = Date.now() - time

    toast({
      status: 'success',
      title: 'Valid Proof',
      description: `Proof was verified in ${diff} ms`,
      duration: 9000,
    })
  })

  return (
    <HStack height='100vh' spacing={4} py={4} mb='-8em'>
      <Stack height='100%' width='100%'>
        <Flex>
          <HStack spacing={4} pb={1}>
            <HStack>
              <Link href='/'>
                <Logo size={26} fontSize='lg' />
              </Link>
              <Heading fontSize='lg'>/ Playground</Heading>
            </HStack>
            <HStack>
              <Select size='sm' defaultValue={EXAMPLES[0].name} borderRadius={5} onChange={(e) => {
                const example = EXAMPLES.find(({ name }) => name === e.target.value)
                setCode(example?.code ?? '')
                setInputs(example?.inputs ?? '')
                clearOutput.execute()
              }}>
                {EXAMPLES.map(({ name }) => <option key={name} value={name}>{name}</option>)}
              </Select>
            </HStack>
          </HStack>
          <Spacer />
          <HStack spacing={2}>
            <Link href='/docs'>Docs</Link>
            <Button size='sm' onClick={prove_browser.execute}>Prove (Browser)</Button>
            <Button size='sm' onClick={prove_server.execute}>Prove (Server)</Button>
            <Button size='sm' onClick={verify.execute}>Verify</Button>
          </HStack>
        </Flex>
        <Alert status='info'>
          <AlertIcon />
          <Text fontSize='xs'>Maximum RAM for the browser prover is 2GB. For larger proofs, consider using the server prover.</Text>
        </Alert>
        <HStack height='100%' spacing={4}>
          <Box width='100%' height='100%' maxW='60%'>
            <Panel heading='Code'>
              <Code type='polylang' value={code} onChange={(code) => {
                setCode(code)
                clearOutput.execute()
              }} />
            </Panel>
          </Box>
          <Stack width='100%' height='100%' flexDirection='column' spacing={4} maxW='50%'>
            <Box height='40%'>
              <Panel heading='Inputs'>
                <Code type='json' value={inputs} onChange={(inputs) => {
                  setInputs(inputs)
                  clearOutput.execute()
                }} />
              </Panel>
            </Box>
            <Box height='100%' borderRadius={5} overflow='hidden'>
              <Panel heading='Output'>
                <Code type='json' editable={false} value={report} />
              </Panel>
            </Box>
          </Stack>
        </HStack>
      </Stack>
    </HStack>
  )
}

export default Playground