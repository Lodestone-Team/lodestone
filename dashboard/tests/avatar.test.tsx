import React from 'react';
import { render, fireEvent } from '@testing-library/react';

import Avatar from '../src/components/Atoms/Avatar';

describe('Avatar Component', () => {
  it('renders correctly with default props', () => {
    const { container, getByText } = render(<Avatar name="John Doe" />);
    expect(container.firstChild).toBeInTheDocument();
  });

  it('renders with custom size and variant', () => {
    const { container } = render(<Avatar name="John Doe" size={30} variant="marble" />);
    expect(container.firstChild).toHaveClass('w-[30px] h-[30px]');
  });

  it('applies custom colors', () => {
    const customColors = ['#FF0000', '#00FF00', '#0000FF'];
    const { container } = render(<Avatar name="John Doe" colors={customColors} />);
    const rects = container.querySelector('rect'); 

    expect(rects).toBeInTheDocument();

    expect(rects).toHaveAttribute('fill', '#FFFFFF');
    expect(rects).toHaveAttribute('width', '36');
    expect(rects).toHaveAttribute('height', '36');
    expect(rects).toHaveAttribute('rx', '72');

  });

});