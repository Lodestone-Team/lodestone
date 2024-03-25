import React from 'react';
import { render, fireEvent, screen } from '@testing-library/react';
import { Toggle } from '../src/components/Atoms/Toggle'; // Assuming this component is in the same directory

describe('Toggle Component', () => {
  it('renders correctly with default props', () => {
    const onChange = jest.fn();
    const { container, getAllByRole } = render(<Toggle value={false} onChange={onChange}></Toggle>);
    
    const span = container.querySelector('span');
    
    // Assert the initial state
    expect(span).toBeInTheDocument();
    expect(span).toBeVisible();
    expect(span).toHaveClass('inline-block h-4 w-4 transform');
    expect(span).toHaveClass('bg-white');
    expect(span).not.toHaveClass('bg-gray-faded/40');

    const button = getAllByRole('switch');

    expect(button.length).toBe(1);
    
    // Click the toggle and check if onChange is called
    fireEvent.click(button[0]);
    expect(onChange).toHaveBeenCalled();
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('renders correctly when disabled', () => {
    const onChange = jest.fn();
    const { container, getAllByRole } = render(<Toggle value={false} onChange={onChange} disabled={true} />);
    const span = container.querySelector('span');
    
    // Assert the initial state
    expect(span).toBeInTheDocument();
    expect(span).toBeVisible();
    expect(span).toHaveClass('inline-block h-4 w-4 transform');
    expect(span).toHaveClass('bg-gray-faded/40');
    expect(span).not.toHaveClass('bg-white');

    const button = getAllByRole('switch');

    expect(button.length).toBe(1);
    
    // Click the toggle and check if onChange is called
    fireEvent.click(button[0]);
    expect(onChange).not.toHaveBeenCalled();
  });
});