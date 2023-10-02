import CodeMirror, { ReactCodeMirrorProps } from '@uiw/react-codemirror'
import { ViewUpdate } from '@codemirror/view'
import { githubLight } from '@uiw/codemirror-theme-github'
import { vscodeDark } from '@uiw/codemirror-theme-vscode'
import { useTheme } from 'nextra-theme-docs'
import { javascript } from '@polybase/codemirror-lang-javascript'
import { json } from '@codemirror/lang-json'

export interface CodeProps extends ReactCodeMirrorProps {
  type: 'json' | 'polylang'
  value: string,
  onChange?: (value: string, update: ViewUpdate) => void
}

export function Code({ value, onChange, type, ...props }: CodeProps) {
  const theme = useTheme()

  return (
    <CodeMirror
      theme={theme.resolvedTheme === 'dark' ? vscodeDark : githubLight}
      style={{
        flex: '1 1 auto',
        height: '100%',
        width: '100%',
        overflow: 'auto',
        fontSize: '0.9em',
      }}
      value={value}
      onChange={onChange}
      height='100%'
      extensions={[type === 'json' ? json() : javascript({ typescript: true })]}
      {...props}
    />
  )
}