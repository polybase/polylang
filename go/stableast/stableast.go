package stableast

import (
	"encoding/json"
	"fmt"
)

type AnyKinded struct {
	Kind  string
	Value json.RawMessage
}

func (a *AnyKinded) UnmarshalJSON(data []byte) error {
	var m struct {
		Kind string `json:"kind"`
	}
	if err := json.Unmarshal(data, &m); err != nil {
		return err
	}

	a.Kind = m.Kind
	a.Value = data

	return nil
}

func (a *AnyKinded) MarshalJSON() ([]byte, error) {
	return a.Value, nil
}

type Root []RootNode

type RootNode AnyKinded

func (a *RootNode) UnmarshalJSON(data []byte) error {
	return json.Unmarshal(data, (*AnyKinded)(a))
}

func (a *RootNode) MarshalJSON() ([]byte, error) {
	return json.Marshal((*AnyKinded)(a))
}

type Collection struct {
	Namespace  Namespace             `json:"namespace"`
	Name       string                `json:"name"`
	Attributes []CollectionAttribute `json:"attributes"`
}

type Namespace struct {
	Value string `json:"value"`
}

func (n *Namespace) UnmarshalJSON(data []byte) error {
	var m struct {
		Kind  string `json:"kind"`
		Value string `json:"value"`
	}
	if err := json.Unmarshal(data, &m); err != nil {
		return err
	}

	if m.Kind != "namespace" {
		return fmt.Errorf("expected kind to be namespace, got %s", m.Kind)
	}

	n.Value = m.Value

	return nil
}

func (n *Namespace) MarshalJSON() ([]byte, error) {
	return json.Marshal(struct {
		Kind  string `json:"kind"`
		Value string `json:"value"`
	}{
		Kind:  "namespace",
		Value: n.Value,
	})
}

func (rn *RootNode) Collection() (*Collection, bool, error) {
	if rn.Kind != "collection" {
		return nil, false, nil
	}

	var c Collection
	if err := json.Unmarshal(rn.Value, &c); err != nil {
		return nil, false, err
	}

	return &c, true, nil
}

type CollectionAttribute AnyKinded

func (a *CollectionAttribute) UnmarshalJSON(data []byte) error {
	return json.Unmarshal(data, (*AnyKinded)(a))
}

func (a *CollectionAttribute) MarshalJSON() ([]byte, error) {
	return json.Marshal((*AnyKinded)(a))
}

type Property struct {
	Name     string `json:"name"`
	Type     Type   `json:"type"`
	Required bool   `json:"required"`
}

type Type AnyKinded

func (a *Type) UnmarshalJSON(data []byte) error {
	return json.Unmarshal(data, (*AnyKinded)(a))
}

func (a *Type) MarshalJSON() ([]byte, error) {
	return json.Marshal((*AnyKinded)(a))
}

type PrimitiveType string

const (
	PrimitiveTypeString  PrimitiveType = "string"
	PrimitiveTypeNumber  PrimitiveType = "number"
	PrimitiveTypeBoolean PrimitiveType = "boolean"
)

func (pt PrimitiveType) IsString() bool {
	return pt == PrimitiveTypeString
}

func (pt PrimitiveType) IsNumber() bool {
	return pt == PrimitiveTypeNumber
}

func (pt PrimitiveType) IsBoolean() bool {
	return pt == PrimitiveTypeBoolean
}

type Primitive struct {
	Value PrimitiveType `json:"value"`
}

func (t *Type) Primitive() (*Primitive, bool, error) {
	if t.Kind != "primitive" {
		return nil, false, nil
	}

	var p Primitive
	if err := json.Unmarshal(t.Value, &p); err != nil {
		return nil, false, err
	}

	return &p, true, nil
}

type Array struct {
	Value Type `json:"value"`
}

func (t *Type) Array() (*Array, bool, error) {
	if t.Kind != "array" {
		return nil, false, nil
	}

	var a Array
	if err := json.Unmarshal(t.Value, &a); err != nil {
		return nil, false, err
	}

	return &a, true, nil
}

type Map struct {
	Key   Type `json:"key"`
	Value Type `json:"value"`
}

func (t *Type) Map() (*Map, bool, error) {
	if t.Kind != "map" {
		return nil, false, nil
	}

	var m Map
	if err := json.Unmarshal(t.Value, &m); err != nil {
		return nil, false, err
	}

	return &m, true, nil
}

type Object struct {
	Fields []ObjectField `json:"fields"`
}

type ObjectField struct {
	Name     string `json:"name"`
	Type     Type   `json:"type"`
	Required bool   `json:"required"`
}

func (t *Type) Object() (*Object, bool, error) {
	if t.Kind != "object" {
		return nil, false, nil
	}

	var o Object
	if err := json.Unmarshal(t.Value, &o); err != nil {
		return nil, false, err
	}

	return &o, true, nil
}

type Record struct{}

func (r *Record) MarshalJSON() ([]byte, error) {
	return []byte(`{}`), nil
}

func (t *Type) Record() (*Record, bool, error) {
	if t.Kind != "record" {
		return nil, false, nil
	}

	var r Record
	if err := json.Unmarshal(t.Value, &r); err != nil {
		return nil, false, err
	}

	return &r, true, nil
}

type ForeignRecord struct {
	Collection string `json:"collection"`
}

func (t *Type) ForeignRecord() (*ForeignRecord, bool, error) {
	if t.Kind != "foreignrecord" {
		return nil, false, nil
	}

	var r ForeignRecord
	if err := json.Unmarshal(t.Value, &r); err != nil {
		return nil, false, err
	}

	return &r, true, nil
}

func (ca *CollectionAttribute) Property() (*Property, bool, error) {
	if ca.Kind != "property" {
		return nil, false, nil
	}

	var p Property
	if err := json.Unmarshal(ca.Value, &p); err != nil {
		return nil, false, err
	}

	return &p, true, nil
}

type Method struct {
	Name       string            `json:"name"`
	Attributes []MethodAttribute `json:"attributes"`
	Code       string            `json:"code"`
}

type MethodAttribute AnyKinded

func (a *MethodAttribute) UnmarshalJSON(data []byte) error {
	return json.Unmarshal(data, (*AnyKinded)(a))
}

func (a *MethodAttribute) MarshalJSON() ([]byte, error) {
	return json.Marshal((*AnyKinded)(a))
}

type Directive struct {
	Name       string               `json:"name"`
	Parameters []DirectiveParameter `json:"parameters"`
}

type DirectiveParameter AnyKinded

func (a *DirectiveParameter) UnmarshalJSON(data []byte) error {
	return json.Unmarshal(data, (*AnyKinded)(a))
}

func (a *DirectiveParameter) MarshalJSON() ([]byte, error) {
	return json.Marshal((*AnyKinded)(a))
}

func (dp *DirectiveParameter) Primitive() (*Primitive, bool, error) {
	if dp.Kind != "primitive" {
		return nil, false, nil
	}

	var p Primitive
	if err := json.Unmarshal(dp.Value, &p); err != nil {
		return nil, false, err
	}

	return &p, true, nil
}

func (ma *MethodAttribute) Directive() (*Directive, bool, error) {
	if ma.Kind != "directive" {
		return nil, false, nil
	}

	var d Directive
	if err := json.Unmarshal(ma.Value, &d); err != nil {
		return nil, false, err
	}

	return &d, true, nil
}

type Parameter struct {
	Name     string `json:"name"`
	Type     Type   `json:"type"`
	Required bool   `json:"required"`
}

func (ma *MethodAttribute) Parameter() (*Parameter, bool, error) {
	if ma.Kind != "parameter" {
		return nil, false, nil
	}

	var p Parameter
	if err := json.Unmarshal(ma.Value, &p); err != nil {
		return nil, false, err
	}

	return &p, true, nil
}

type ReturnValue struct {
	Name string `json:"name"`
	Type Type   `json:"type"`
}

func (ma *MethodAttribute) ReturnValue() (*ReturnValue, bool, error) {
	if ma.Kind != "returnvalue" {
		return nil, false, nil
	}

	var rv ReturnValue
	if err := json.Unmarshal(ma.Value, &rv); err != nil {
		return nil, false, err
	}

	return &rv, true, nil
}

func (ca *CollectionAttribute) Method() (*Method, bool, error) {
	if ca.Kind != "method" {
		return nil, false, nil
	}

	var m Method
	if err := json.Unmarshal(ca.Value, &m); err != nil {
		return nil, false, err
	}

	return &m, true, nil
}

type Index struct {
	Fields []IndexField `json:"fields"`
}

type IndexField struct {
	Direction Order    `json:"direction"`
	FieldPath []string `json:"fieldPath"`
}

type Order string

func (o Order) Asc() bool {
	return o == "asc"
}

func (o Order) Desc() bool {
	return o == "desc"
}

func (ca *CollectionAttribute) Index() (*Index, bool, error) {
	if ca.Kind != "index" {
		return nil, false, nil
	}

	var i Index
	if err := json.Unmarshal(ca.Value, &i); err != nil {
		return nil, false, err
	}

	return &i, true, nil
}
