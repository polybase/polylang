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

type Type string

const (
	String Type = "String"
	Number Type = "Number"
)

type Function struct {
	Name       string        `json:"name"`
	Parameters []string      `json:"parameters"`
	Statements []interface{} `json:"statements"`
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
