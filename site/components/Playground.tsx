import { Box, Stack, HStack, Flex, Heading, Spacer, Button, Select, useToast } from '@chakra-ui/react'
import { Code } from './Code'
import { Logo } from './Logo'
import { useState } from 'react'
import { EXAMPLES } from './example'
import Link from 'next/link'
import { useAsyncCallback } from './useAsyncCallback'
import { compile, run, Output } from './polylang'
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
  const [output, setOutput] = useState<Output | null>(null)
  const toast = useToast()

  // Uses the /prove endpoint
  const prove_server = useAsyncCallback(() => {
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
        setOutput(output)

        console.log('server output = ', output)

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

  // Uses the /verify endpoint
  const verify_server = useAsyncCallback(() => {
    if (!output) {
      return toast({
        status: 'error',
        title: 'No proof',
        description: 'There is no proof to verify',
        duration: 9000,
      })
    }

    const time = Date.now()
    const verifierUrl = 'http://127.0.0.1:8080/verify'

    const verifyPayload = {
      proof: output.proof,
      programInfo: output.programInfo,
      stackInputs: output.stack.input,
      outputStack: output.stack.output,
      overflowAddrs: output.stack.overflowAddrs,
    }

    fetch(verifierUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(verifyPayload),
    }).then(resp => {
      console.log(resp)
      const diff = Date.now() - time

      toast({
        status: 'success',
        title: 'Valid Proof',
        description: `Proof was verified in ${diff} ms`,
        duration: 9000,
      })
    }).catch(err => {
      console.log(err)
    })
  })

  // Uses the WASM API
  const prove_browser = useAsyncCallback(async () => {
    const parsedInputs = JSON.parse(inputs)
    const output = run(code, parsedInputs)
    setOutput(output)

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

  // Uses the WASM API
  const verify_browser = useAsyncCallback(() => {
    if (!output) {
      return toast({
        status: 'error',
        title: 'No proof',
        description: 'There is no proof to verify',
        duration: 9000,
      })
    }
    const time = Date.now()
    output?.verify()
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
                setOutput(null)
                setReport('')
              }}>
                {EXAMPLES.map(({ name }) => <option key={name} value={name}>{name}</option>)}
              </Select>
            </HStack>
          </HStack>
          <Spacer />
          <HStack spacing={2}>
            <Link href='/docs'>Docs</Link>
            <Button size='sm' onClick={prove_browser.execute}>Prove (Browser)</Button>
            <Button size='sm' onClick={verify_browser.execute}>Verify (Browser)</Button>
            <Button size='sm' onClick={prove_server.execute}>Prove (Server)</Button>
            <Button size='sm' onClick={verify_server.execute}>Verify (Server)</Button>
          </HStack>
        </Flex>
        <HStack height='100%' spacing={4}>
          <Box width='100%' height='100%' maxW='60%'>
            <Panel heading='Code'>
              <Code type='polylang' value={code} onChange={(code) => {
                setCode(code)
                setOutput(null)
                setReport('')
              }} />

            </Panel>
          </Box>
          <Stack width='100%' height='100%' flexDirection='column' spacing={4} maxW='50%'>
            <Box height='40%'>
              <Panel heading='Inputs'>
                <Code type='json' value={inputs} onChange={(inputs) => {
                  setInputs(inputs)
                  setOutput(null)
                  setReport('')
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