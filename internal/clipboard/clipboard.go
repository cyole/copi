package clipboard

type Provider interface {
	ReadText() (string, error)
	WriteText(string) error
}

type System struct{}

func NewSystem() System {
	return System{}
}
