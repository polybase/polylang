package ast

type Program struct {
	Nodes []RootNode `json:"nodes"`
}

type RootNode struct {
	Collection *Collection
	Function   *Function
}

type Collection struct {
	Name  string           `json:"name"`
	Items []CollectionItem `json:"items"`
}

type CollectionItem struct {
	Field    *Field    `json:"Field,omitempty"`
	Function *Function `json:"Function,omitempty"`
	Index    *Index    `json:"Index,omitempty"`
}

type Field struct {
	Name     string `json:"name"`
	Type     Type   `json:"type_"`
	Required bool   `json:"required"`
}

type Type struct {
	Tag     string      `json:"tag"`
	Content interface{} `json:"content,omitempty"`
}

func (t *Type) IsString() bool {
	return t.Tag == "String"
}

func (t *Type) IsNumber() bool {
	return t.Tag == "Number"
}

func (t *Type) IsArray() bool {
	return t.Tag == "Array"
}

type FieldDecorator struct {
	Name      string      `json:"name"`
	Arguments []Primitive `json:"arguments"`
}

type Function struct {
	Name           string        `json:"name"`
	Parameters     []Parameter   `json:"parameters"`
	ReturnType     *Type         `json:"return_type"`
	Statements     []interface{} `json:"statements"`
	StatementsCode string        `json:"statements_code"`
}

type FunctionType struct {
	Tag     string      `json:"tag"`
	Content interface{} `json:"content,omitempty"`
}

func (ft *FunctionType) IsString() bool {
	return ft.Tag == "String"
}

func (ft *FunctionType) IsNumber() bool {
	return ft.Tag == "Number"
}

func (ft *FunctionType) IsRecord() bool {
	return ft.Tag == "Record"
}

func (ft *FunctionType) IsArray() bool {
	return ft.Tag == "Array"
}

func (ft *FunctionType) IsMap() bool {
	return ft.Tag == "Map"
}

type Parameter struct {
	Name     string       `json:"name"`
	Type     FunctionType `json:"type_"`
	Required bool         `json:"required"`
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
