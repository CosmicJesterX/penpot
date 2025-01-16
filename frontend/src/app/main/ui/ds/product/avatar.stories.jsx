import * as React from "react";
import Components from "@target/components";

const { Avatar } = Components;

export default {
  title: "Product/Avatar",
  component: Avatar,
  argTypes: {
    title: {
      control: { type: "text" },
    }
  },
  args: {
    title: "Avatar"
  },
  render: ({ ...args }) => <Avatar {...args}/>,
};

export const Default = {};

