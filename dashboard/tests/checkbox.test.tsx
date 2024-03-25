import React from 'react';
import { render, fireEvent } from '@testing-library/react';
import Checkbox from '../src/components/Atoms/Checkbox';

describe('Checkbox Component', () => {
  it('renders unchecked checkbox with label', () => {
    const { container } = render(<Checkbox label="Checkbox Label" checked={false} onChange={() => {}} />);

    const checkboxContainer = container.getElementsByClassName('flex items-center gap-3 text-medium font-medium tracking-medium');

    expect(checkboxContainer.length).toBe(1);
    expect(checkboxContainer[0].children.length).toBe(2);

    const labelElement = checkboxContainer[0].querySelector('label');
    expect(checkboxContainer[0]).toBeInTheDocument(); // Ensure checkbox container is rendered
    expect(labelElement?.textContent).toBe('Checkbox Label'); // Ensure label is rendered correctly
    expect(checkboxContainer[0].children[0]).toHaveClass('text-gray-400'); // Ensure unchecked style is applied
  });

  it('renders checked checkbox with label', () => {
    const { container } = render(<Checkbox label="Checkbox Label" checked={true} onChange={() => {}} />);
    const checkboxContainer = container.getElementsByClassName('flex items-center gap-3 text-medium font-medium tracking-medium')[0];
    const labelElement = checkboxContainer.querySelector('label');
    expect(checkboxContainer).toBeInTheDocument(); // Ensure checkbox container is rendered
    expect(labelElement?.textContent).toBe('Checkbox Label'); // Ensure label is rendered correctly
    expect(checkboxContainer.children[0]).toHaveClass('text-gray-300'); // Ensure checked style is applied
  });

  it('calls onChange handler when clicked', () => {
    const onChange = jest.fn();
    const { container } = render(<Checkbox label="Checkbox Label" checked={false} onChange={onChange} />);
    const checkboxContainer = container.getElementsByClassName('flex items-center gap-3 text-medium font-medium tracking-medium')[0];
    fireEvent.click(checkboxContainer.children[0]);
    expect(onChange).toHaveBeenCalledTimes(1); // Ensure onChange is called once
    expect(onChange).toHaveBeenCalledWith(true); // Ensure onChange is called with correct value
  });

  it('does not call onChange handler when clicked if disabled', () => {
    const onChange = jest.fn();
    const { container } = render(<Checkbox label="Checkbox Label" checked={false} onChange={onChange} disabled />);
    const checkboxContainer = container.getElementsByClassName('flex items-center gap-3 text-medium font-medium tracking-medium')[0];
    fireEvent.click(checkboxContainer);
    expect(onChange).not.toHaveBeenCalled(); // Ensure onChange is not called
  });

  it('applies disabled styles when disabled', () => {
    const { container } = render(<Checkbox label="Checkbox Label" checked={false} onChange={() => {}} disabled />);
    const checkboxContainer = container.getElementsByClassName('flex items-center gap-3 text-medium font-medium tracking-medium')[0];
    expect(checkboxContainer.children[0]).toHaveClass('text-gray-500'); // Ensure disabled style is applied
  });

  it('does not apply disabled styles when not disabled', () => {
    const { container } = render(<Checkbox label="Checkbox Label" checked={false} onChange={() => {}} />);
    const checkboxContainer = container.getElementsByClassName('flex items-center gap-3 text-medium font-medium tracking-medium')[0];
    expect(checkboxContainer).not.toHaveClass('text-gray-500'); // Ensure disabled style is not applied
  });

});
