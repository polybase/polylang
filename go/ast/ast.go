package ast

type Program struct {
	Nodes []RootNode `json:"nodes"`
}

type RootNode struct {
	Contract *Contract
	Function *Function
}

type Contract struct {
	Name  string         `json:"name"`
	Items []ContractItem `json:"items"`
}

type ContractItem struct {
	Field    *Field    `json:"Field,omitempty"`
	Function *Function `json:"Function,omitempty"`
	Index    *Index    `json:"Index,omitempty"`
}

type Field struct {
	Name       string           `json:"name"`
	Type       Type             `json:"type_"`
	Required   bool             `json:"required"`
	Decorators []FieldDecorator `json:"decorators"`
}

type Type string

const (
	String Type = "String"
	Number Type = "Number"
)

type FieldDecorator struct {
	Name      string      `json:"name"`
	Arguments []Primitive `json:"arguments"`
}

type Function struct {
	Name           string        `json:"name"`
	Parameters     []Parameter   `json:"parameters"`
	Statements     []interface{} `json:"statements"`
	StatementsCode string        `json:"statements_code"`
}

type FunctionType string

const (
	FunctionTypeString FunctionType = "String"
	FunctionTypeNumber FunctionType = "Number"
	FunctionTypeRecord FunctionType = "Record"
)

type Parameter struct {
	Name string       `json:"name"`
	Type FunctionType `json:"type_"`
}

type Index struct {
	Unique bool         `json:"unique"`
	Fields []IndexField `json:"fields"`
}

type IndexField struct {
	Name  string `json:"name"`
	Order Order  `json:"order"`
}

type Order string

const (
	Asc  Order = "Asc"
	Desc Order = "Desc"
)

type Primitive struct {
	Number *float64 `json:"Number,omitempty"`
	String *string  `json:"String,omitempty"`
	Regex  *string  `json:"Regex,omitempty"`
}
