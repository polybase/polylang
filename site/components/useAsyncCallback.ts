import { useAsyncCallback as useAsyncCallbackBase, UseAsyncCallbackOptions } from 'react-async-hook'
import { useToast } from '@chakra-ui/react'

export type ExtendedOptions<R> = UseAsyncCallbackOptions<R> & {
  logError?: boolean
  errorTitle?: string
  successTitle?: string
}

export function useAsyncCallback<R = unknown, Args extends any[] = any[]>(asyncFunction: (...args: Args) => Promise<R> | R, options?: ExtendedOptions<R>) {
  const { onError, errorTitle, logError, successTitle, ...baseOptions } = options || {}
  const toast = useToast()
  return useAsyncCallbackBase(async (...params: Args) => {
    const res = await asyncFunction(...params)
    if (successTitle) {
      toast({
        status: 'success',
        title: successTitle,
      })
    }
    return res
  }, {
    ...baseOptions,
    onError: (e, options) => {
      if (onError) onError(e, options)

      if (e?.message === 'unreachable') {
        e.message = 'Proof aborted, program uses too much RAM for browser environment'
      }


      toast({
        status: 'error',
        title: errorTitle,
        description: e?.message,
        isClosable: true,
        duration: 9000,
      })
    },
  })
}
