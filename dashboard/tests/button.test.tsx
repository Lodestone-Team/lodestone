import React from 'react';
import { render, fireEvent } from '@testing-library/react';
import Button from '../src/components/Atoms/Button';

// Mock the functions imported from 'utils/util'
jest.mock('utils/util', () => ({
  cn: jest.fn((...args) => args.join(' ')),
  myTwMerge: jest.fn((...args) => args.join(' ')),
}));

describe('Button Component', () => {
  afterEach(() => {
    jest.clearAllMocks(); // Reset mock calls between tests
  });

  it('renders button with label and icon', () => {
    const { getByText, getByTestId } = render(
      <Button
        label="Button Label"
        iconComponent={<span data-testid="mock-icon">Icon</span>}
      />
    );

    const button = getByText('Button Label');
    const icon = getByTestId('mock-icon');

    expect(button).toBeInTheDocument(); // Ensure button is rendered
    expect(icon).toBeInTheDocument(); // Ensure icon is rendered
    expect(icon.textContent).toBe('Icon'); // Ensure icon content is correct
  });

  it('handles click events correctly', () => {
    const onClick = jest.fn(); // Mock onClick function
    const { getByText } = render(<Button label="Button Label" onClick={onClick} />);

    const button = getByText('Button Label');
    fireEvent.click(button); // Simulate click event

    expect(onClick).toHaveBeenCalledTimes(1); // Ensure onClick function is called once
  });

  it('disables button when disabled prop is true', () => {
    const { container } = render(<Button label="Button Label" disabled />);

    const button = container.querySelector('button');

    expect(button).toHaveClass('group flex select-none flex-row flex-nowrap items-center leading-normal tracking-medium enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50 justify-center gap-1.5 rounded py-1 px-2 text-medium text-gray-300 disabled:text-white/50 font-medium bg-gray-800 enabled:hover:bg-gray-700 enabled:active:bg-gray-800 enabled:ui-active:bg-gray-700 outline-gray-faded/30 enabled:hover:outline-white/50 outline outline-1'); // Ensure button is rendered

    expect(button).toBeDisabled(); // Ensure button is disabled
  });

  it('applies custom class names correctly', () => {
    const { container } = render(<Button label="Button Label" className="custom-class" />);

    const button = container.querySelector('button');
    expect(button).toHaveClass('custom-class'); // Ensure custom class is applied
  });

  it('applies loading state correctly', () => {
    const { container } = render(<Button label="Button Label" loading />);

    const loader = container.getElementsByClassName('absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 transform');
    expect(loader.length).toBe(1); // Ensure loader is rendered
    expect(loader[0]).toBeInTheDocument(); // Ensure loader is rendered

    const button = container.querySelector('button');
    expect(button).toBeDisabled(); // Ensure button is disabled
  });

});
